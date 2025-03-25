/// TODO: Seperate the graph generation logic into a separate module
/// TODO: Add tests for the graph generation logic
use mlua::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

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

/// Execute shell command and collect output asynchronously
///
/// ## Parameters
/// * `cmd` - The command to execute
/// * `args` - The arguments to pass to the command
///
/// ## Returns
/// The output of the command as a string
async fn get_forward_links_async(filepath: &str, base_dir: &str) -> Vec<String> {
    let pattern = "\\[.*?\\]\\((.*?\\.md)\\)";
    let args = vec!["-o", "--no-line-number", pattern, filepath];
    let output = execute_command_async("rg", &args).await;
    parse_links(&output, base_dir)
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
    let filename = Path::new(filepath)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let directory = Path::new(filepath)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let pattern = format!("^.*\\[.*?\\]\\((./{})\\)", filename);
    let args = vec!["-o", "--no-line-number", &pattern, &directory];
    let output = execute_command_async("rg", &args).await;
    parse_file_paths(&output, base_dir)
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
    let pattern = regex::Regex::new(r"\[.*?\]\((.*?\.md)\)").unwrap();

    for cap in pattern.captures_iter(output) {
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
        // Use memchr for faster substring search
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
    // Use more efficient hash maps with faster hashing algorithm
    let mut all_links: HashMap<String, (Vec<String>, HashMap<String, u32>), fnv::FnvBuildHasher> =
        HashMap::with_hasher(fnv::FnvBuildHasher::default());
    let mut node_distances: HashMap<String, u32, fnv::FnvBuildHasher> =
        HashMap::with_hasher(fnv::FnvBuildHasher::default());
    let mut visited: HashSet<String, fnv::FnvBuildHasher> =
        HashSet::with_hasher(fnv::FnvBuildHasher::default());

    let mut current_layer = Vec::with_capacity(100); // Reasonable initial capacity
    current_layer.push(Node {
        filepath: start_file.to_string(),
        distance: 0,
    });
    visited.insert(start_file.to_string());

    for _ in 0..max_depth {
        // Process nodes in parallel with configurable batch size
        let batch_size = 16; // Optimize based on your workload
        let mut tasks = Vec::with_capacity(current_layer.len());

        for nodes_chunk in current_layer.chunks(batch_size) {
            for node in nodes_chunk {
                tasks.push(process_node_async(node, base_dir));
            }
        }

        let results = futures::future::join_all(tasks).await;
        current_layer = Vec::with_capacity(results.len() * 4); // Estimate growth factor

        for result in results {
            for (target, sources) in result.backward_links {
                all_links
                    .entry(target.clone())
                    .or_insert_with(|| (Vec::with_capacity(sources.len()), HashMap::default()))
                    .0
                    .extend(sources.clone());

                // Track the distance for each source to target
                for source in sources {
                    let current_distance = node_distances.get(&source).copied().unwrap_or(0) + 1;
                    all_links
                        .entry(target.clone())
                        .or_insert_with(|| (Vec::new(), HashMap::default()))
                        .1
                        .insert(source.clone(), current_distance);
                }
            }

            for new_node in result.new_nodes {
                if !visited.contains(&new_node.filepath) {
                    visited.insert(new_node.filepath.clone());
                    node_distances.insert(new_node.filepath.clone(), new_node.distance);
                    current_layer.push(new_node);
                }
            }
        }

        if current_layer.is_empty() {
            break;
        }
    }

    // Convert to standard HashMap for compatibility
    all_links.into_iter().collect()
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
    let forward = get_forward_links_async(&node.filepath, base_dir).await;
    let backward = get_backward_links_async(&node.filepath, base_dir).await;

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
    // Use a HashMap to track the shortest distance for each node
    let mut shortest_paths: HashMap<String, u32> = HashMap::new();

    // Set the start file distance to 0
    shortest_paths.insert(start_file.to_string(), 0);

    for (target, (sources, distances)) in links {
        for source in sources {
            if source != start_file {
                let distance = distances.get(&source).copied().unwrap_or(1);
                shortest_paths
                    .entry(source)
                    .and_modify(|dist| *dist = (*dist).min(distance))
                    .or_insert(distance);
            }
        }
    }
    shortest_paths.into_iter().collect()
}

#[mlua::lua_module]
fn note_tree(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    let _ = exports.set(
        "generate_double_chain_graph",
        lua.create_function(
            |lua, (start_node, max_distance, base_dir): (LuaTable, u32, String)| {
                // Extract start node data from Lua table
                let filepath: String = start_node.get("filepath")?;
                let _filename: String = start_node.get("filename")?;

                // Create and run a runtime for the async operations
                let rt = tokio::runtime::Runtime::new()?;
                let links = rt.block_on(generate_graph_async(&filepath, max_distance, &base_dir));

                // Generate the paths data using our efficient function
                let paths_data = build_shortest_paths_data(&filepath, links);

                // Convert to Lua table
                let result_array = lua
                    .create_table_with_capacity((paths_data.len() as i32).try_into().unwrap(), 0)?;

                for (i, (node, path_length)) in paths_data.into_iter().enumerate() {
                    let entry = lua.create_table_with_capacity(0, 2)?;
                    entry.set("node", node)?;
                    entry.set("path_length", path_length)?;
                    result_array.raw_set(i as i64 + 1, entry)?;
                }

                Ok(result_array)
            },
        )?,
    );

    Ok(exports)
}
