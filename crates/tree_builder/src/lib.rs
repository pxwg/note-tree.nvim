/// TODO: Seperate the graph generation logic into a separate module
/// TODO: Add tests for the graph generation logic
use fnv::FnvBuildHasher;
use lazy_static::lazy_static;
use log::{debug, info, warn};
use mlua::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::io::{self, AsyncBufRead, AsyncBufReadExt};
use tokio::sync::mpsc;
use tokio::task;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct Node {
    filepath: String,
    distance: u32,
}

struct ProcessResult {
    new_nodes: Vec<Node>,
    backward_links: HashMap<String, Vec<String>>,
}

#[derive(Clone)]
struct DoubleChainNode {
    filepath: String,
    filename: String,
}

// Structure for the graph entry
struct DoubleChainGraph {
    node: DoubleChainNode,
    distance: u32,
}

// Structure for the output of the graph
struct ShortestPath {
    node: String,
    path_: u32,
}

/// Initialize the rust log
pub fn initialize() {
    use env_logger::{Builder, Target};
    use std::env;
    use std::fs::File;
    use std::io::Write;

    let log_path =
        env::var("TREE_BUILDER_LOG_PATH").unwrap_or_else(|_| "tree_builder.log".to_string());

    let file = File::create(&log_path).expect("Failed to create log file");

    let mut builder = Builder::new();
    builder.target(Target::Pipe(Box::new(file)));

    if let Ok(env_filter) = env::var("RUST_LOG") {
        builder.parse_filters(&env_filter);
    } else {
        builder.filter_level(log::LevelFilter::Info);
    }

    builder.init();

    info!("Tree builder logger initialized at: {}", log_path);
}

/// Convert relative paths to absolute paths
/// ## Arguments
/// * `path` - The path to convert
/// * `base_dir` - The base directory to use
/// ## Returns
/// The absolute path
fn convert_to_absolute_path(path: &str, base_dir: &str) -> String {
    let path_obj = Path::new(path);
    if path_obj.is_absolute() {
        path.to_string()
    } else {
        Path::new(base_dir)
            .join(path.strip_prefix("./").unwrap_or(path))
            .to_string_lossy()
            .into_owned()
    }
}

/// Execute shell command and collect output asynchronously
///
/// ## Parameters
/// * `cmd` - The command to execute
/// * `args` - The arguments to pass to the command
///
/// ## Returns
/// The output of the command as a string
async fn execute_command_async(cmd: &str, args: &[&str]) -> String {
    let cmd = cmd.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

    tokio::task::spawn_blocking(move || {
        Command::new(&cmd)
            .args(&args)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

/// Get all forward links in a file asynchronously
///
/// ## Parameters
/// * `filepath` - The file to search for links
/// * `base_dir` - The base directory to use
///
/// ## Returns
/// A vector of forward links
async fn get_forward_links_async(filepath: &str, base_dir: &str) -> Vec<String> {
    // Compile regex once and reuse
    lazy_static! {
        static ref MARKDOWN_LINK_PATTERN: Regex = Regex::new(r"\[(.*?)\]\((.*?\.md)\)").unwrap();
    }

    // Use async file reading
    let content = match tokio::fs::read_to_string(filepath).await {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };

    // Pre-allocate with reasonable capacity based on typical markdown files
    let mut links = Vec::with_capacity(10);

    // Process line by line with early filtering
    for line in content.lines() {
        // Quick check to skip lines without potential links
        if !line.contains('[') || !line.contains("](") {
            continue;
        }

        // Apply regex only on promising lines
        for cap in MARKDOWN_LINK_PATTERN.captures_iter(line) {
            if let Some(link) = cap.get(2) {
                links.push(convert_to_absolute_path(link.as_str(), base_dir));
            }
        }
    }

    links
}

/// Get all backward links in a file asynchronously
///
/// ## Parameters
/// * `filepath` - The file to search for links
/// * `base_dir` - The base directory to use
///
/// ## Returns
/// A vector of backward links
async fn get_backward_links_async(filepath: &str, base_dir: &str) -> Vec<String> {
    // Extract filename and directory once
    let path = Path::new(filepath);
    let filename = match path.file_name() {
        Some(name) => name.to_string_lossy(),
        None => return Vec::new(),
    };

    let directory = match path.parent() {
        Some(dir) => dir.to_string_lossy().to_string(),
        None => return Vec::new(),
    };

    // Match both direct filename links and links with path components
    let link_pattern = Regex::new(&format!(
        r"\[.*?\]\(\./{}\)",
        regex::escape(filename.as_ref())
    ))
    .unwrap();

    let mut entries = match tokio::fs::read_dir(&directory).await {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut backward_links = Vec::with_capacity(10);

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if path.to_string_lossy() == filepath {
            continue;
        }

        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            // Quick check if file might contain the filename before doing line-by-line search
            if !content.contains(filename.as_ref()) {
                continue;
            }

            for line in content.lines() {
                if !line.contains('[') || !line.contains(']') || !line.contains('(') {
                    continue;
                }

                if link_pattern.is_match(line) {
                    backward_links.push(convert_to_absolute_path(
                        path.to_string_lossy().as_ref(),
                        base_dir,
                    ));
                    break; // Found link, no need to check more lines
                }
            }
        }
    }

    backward_links
}

/// Parse links from ripgrep output
///
/// ## Parameters
/// * `output` - The raw output from ripgrep
/// * `base_dir` - The base directory to use for path conversion
///
/// ## Returns
/// A vector of absolute paths to linked files
fn parse_links(output: &str, base_dir: &str) -> Vec<String> {
    let mut links = Vec::with_capacity(output.lines().count());

    // Pre-compile the pattern for parsing
    lazy_static! {
        static ref LINK_PATTERN: Regex = Regex::new(r"\[.*?\]\((.*?\.md)\)").unwrap();
    }

    for cap in LINK_PATTERN.captures_iter(output) {
        if let Some(link) = cap.get(1) {
            links.push(convert_to_absolute_path(link.as_str(), base_dir));
        }
    }

    links
}

/// Parse file paths from ripgrep output
///
/// ## Parameters
/// * `output` - The raw output from ripgrep
/// * `base_dir` - The base directory to use for path conversion
///
/// ## Returns
/// A vector of absolute file paths
fn parse_file_paths(output: &str, base_dir: &str) -> Vec<String> {
    let mut paths = Vec::with_capacity(output.lines().count());

    for line in output.lines() {
        if let Some(idx) = memchr::memchr(b':', line.as_bytes()) {
            let file_path = &line[..idx];
            paths.push(convert_to_absolute_path(file_path, base_dir));
        }
    }

    paths
}

/// Generate a graph of interconnected markdown files
///
/// ## Parameters
/// * `start_file` - The file path to start the graph from
/// * `max_depth` - The maximum depth of traversal
/// * `base_dir` - The base directory to use for path conversion
///
/// ## Returns
/// A map of target files to their source files and distances
async fn generate_graph_async(
    start_file: &str,
    max_depth: u32,
    base_dir: &str,
) -> HashMap<String, (Vec<String>, HashMap<String, u32>)> {
    type FnvHashMap<K, V> = HashMap<K, V, FnvBuildHasher>;
    type FnvHashSet<K> = HashSet<K, FnvBuildHasher>;

    // Check if start file exists
    if !Path::new(start_file).exists() {
        warn!("Start file does not exist: {}", start_file);
        return HashMap::new();
    }

    let mut all_links: FnvHashMap<String, (Vec<String>, FnvHashMap<String, u32>)> =
        FnvHashMap::default();
    let mut node_distances: FnvHashMap<String, u32> = FnvHashMap::default();
    let mut visited: FnvHashSet<String> = FnvHashSet::default();

    let mut current_layer = Vec::with_capacity(100);
    current_layer.push(Node {
        filepath: start_file.to_string(),
        distance: 0,
    });
    visited.insert(start_file.to_string());
    node_distances.insert(start_file.to_string(), 0);

    for _current_depth in 0..max_depth {
        if current_layer.is_empty() {
            break;
        }

        let mut tasks = Vec::with_capacity(current_layer.len());
        for node in &current_layer {
            tasks.push(process_node_async(node, base_dir));
        }

        let results = futures::future::join_all(tasks).await;
        let mut next_layer = Vec::with_capacity(results.len() * 4); // Estimate growth factor

        for result in results {
            for (target, sources) in &result.backward_links {
                let entry = all_links
                    .entry(target.clone())
                    .or_insert_with(|| (Vec::with_capacity(sources.len()), FnvHashMap::default()));

                entry.0.extend(sources.iter().cloned());

                for source in sources {
                    let source_distance = node_distances.get(source).copied().unwrap_or(0);
                    let target_distance = source_distance + 1;
                    entry.1.insert(source.clone(), target_distance);
                }
            }

            // Add new nodes from the result to the next layer
            for new_node in result.new_nodes {
                if visited.insert(new_node.filepath.clone()) {
                    node_distances.insert(new_node.filepath.clone(), new_node.distance);
                    next_layer.push(new_node);
                }
            }
        }
        current_layer = next_layer;
    }

    // Convert FnvHashMap to standard HashMap to match the return type
    all_links
        .into_iter()
        .map(|(k, (v1, v2))| {
            // Convert inner FnvHashMap to standard HashMap
            let std_v2: HashMap<String, u32> = v2.into_iter().collect();
            (k, (v1, std_v2))
        })
        .collect()
}

/// Process a single node in the graph to find its links
///
/// ## Parameters
/// * `node` - The node to process
/// * `base_dir` - The base directory to use for path conversion
///
/// ## Returns
/// A ProcessResult containing new nodes and backward links
async fn process_node_async(node: &Node, base_dir: &str) -> ProcessResult {
    // Run both forward and backward link searches in parallel
    let (forward, backward) = futures::join!(
        get_forward_links_async(&node.filepath, base_dir),
        get_backward_links_async(&node.filepath, base_dir)
    );

    let new_nodes = forward
        .iter()
        .chain(backward.iter())
        .map(|link| Node {
            filepath: link.to_string(),
            distance: node.distance + 1,
        })
        .collect();

    let mut backward_links = HashMap::new();
    for bl in backward {
        backward_links
            .entry(bl)
            .or_insert_with(Vec::new)
            .push(node.filepath.clone());
    }
    debug!("forward_links: {:?}", new_nodes);
    debug!("backward_links: {:?}", backward_links);

    ProcessResult {
        new_nodes,
        backward_links,
    }
}

/// Converts graph data into a vector of node/path_length pairs
/// ensuring each node appears only once with its shortest path length
/// ## Parameters
/// * `start_file` - The starting file for the graph
/// * `links` - The graph data
/// ## Returns
/// A vector of node/path_length pairs
fn build_shortest_paths_data(
    start_file: &str,
    links: HashMap<String, (Vec<String>, HashMap<String, u32>)>,
) -> Vec<(String, u32)> {
    // info!("Building shortest links from: {:?}", links);
    // info!("Building shortest paths data from: {}", start_file);
    info!("Graph contains {} nodes", links.len());

    // Use a BFS-like approach to calculate shortest paths
    let mut shortest_paths: HashMap<String, u32> = HashMap::new();
    let mut queue = vec![(start_file.to_string(), 0)]; // Start with distance 0 to match graph generation
    shortest_paths.insert(start_file.to_string(), 0);

    // Collect all nodes first to ensure we include isolated nodes
    let mut all_nodes = HashSet::new();
    all_nodes.insert(start_file.to_string());

    for (target, (sources, _)) in &links {
        all_nodes.insert(target.clone());
        for source in sources {
            all_nodes.insert(source.clone());
        }
    }

    debug!("Total unique nodes in graph: {}", all_nodes.len());

    // Process the queue to find shortest paths
    let mut index = 0;
    while index < queue.len() {
        let (current, distance) = queue[index].clone();
        index += 1;

        debug!("Processing node '{}' at distance {}", current, distance);

        // If this node has outgoing links
        if let Some((_, distances)) = links.get(&current) {
            for (target, &target_distance) in distances {
                let new_distance = distance + target_distance;
                match shortest_paths.get(target) {
                    Some(&existing) if existing <= new_distance => {}
                    _ => {
                        debug!("Found shorter path to '{}': {}", target, new_distance);
                        shortest_paths.insert(target.clone(), new_distance);
                        queue.push((target.clone(), new_distance));
                    }
                }
            }
        }
    }

    let result: Vec<(String, u32)> = all_nodes
        .into_iter()
        .map(|node| {
            let distance = shortest_paths.get(&node).copied().unwrap_or(u32::MAX);
            if distance == u32::MAX {
                warn!("Node '{}' is unreachable from start", node);
            }
            (node.clone(), distance)
        })
        .collect();

    info!("Generated {} path entries", result.len());
    result
    // all_nodes.collect()
}

#[mlua::lua_module]
fn note_tree(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;
    initialize();

    let _ = exports.set(
        "generate_double_chain_graph",
        lua.create_function(
            |lua, (start_node, max_distance, base_dir): (LuaTable, u32, String)| {
                // Extract start node data from Lua table
                let filepath: String = start_node.get("filepath")?;
                let _filename: String = start_node.get("filename")?;

                let rt = tokio::runtime::Runtime::new()?;
                let links = rt.block_on(generate_graph_async(&filepath, max_distance, &base_dir));

                let result_array = lua.create_table()?;
                let mut index = 1;

                for (target, (sources, distances)) in links {
                    let target_table = lua.create_table()?;
                    target_table.set("node", target.clone())?;
                    let path_length = if target == filepath { 0 } else { 1 };
                    target_table.set("path_length", path_length)?;
                    result_array.raw_set(index, target_table)?;
                    index += 1;

                    for source in sources {
                        if source != filepath {
                            let source_table = lua.create_table()?;
                            source_table.set("node", source.clone())?;

                            let distance = distances.get(&source).copied().unwrap_or(1);
                            source_table.set("path_length", distance)?;

                            result_array.raw_set(index, source_table)?;
                            index += 1;
                        }
                    }
                }
                Ok(result_array)
            },
        )?,
    );

    Ok(exports)
}
