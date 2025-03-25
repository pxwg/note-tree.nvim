/// TODO: Seperate the graph generation logic into a separate module
/// TODO: Add tests for the graph generation logic
/// TODO: Fully rust api
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
    let mut links = Vec::new();

    for line in output.lines() {
        // Extract the link part from markdown format [text](link.md)
        if let Some(link_part) = line.split('(').nth(1) {
            if let Some(link) = link_part.strip_suffix(')') {
                links.push(convert_to_absolute_path(link, base_dir));
            }
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
    let mut paths = Vec::new();

    for line in output.lines() {
        // Extract the source file path from the ripgrep output
        if let Some(file_path) = line.split(':').next() {
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
    let mut all_links = HashMap::new();
    let mut node_distances = HashMap::new();
    let mut visited = HashSet::new();
    let mut current_layer = vec![Node {
        filepath: start_file.to_string(),
        distance: 0,
    }];
    visited.insert(start_file.to_string());

    for _ in 0..max_depth {
        let mut tasks = Vec::new();
        for node in &current_layer {
            tasks.push(process_node_async(node, base_dir));
        }

        let results = futures::future::join_all(tasks).await;
        current_layer = Vec::new();

        for result in results {
            for (target, sources) in result.backward_links {
                all_links
                    .entry(target.clone())
                    .or_insert_with(|| (Vec::new(), HashMap::new()))
                    .0
                    .extend(sources.clone());

                // Track the distance for each source to target
                for source in sources {
                    let current_distance = node_distances.get(&source).copied().unwrap_or(0) + 1;
                    all_links
                        .entry(target.clone())
                        .or_insert_with(|| (Vec::new(), HashMap::new()))
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

    all_links
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

#[mlua::lua_module]
fn note_tree(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    let _ = exports.set(
        "generate_double_chain_graph",
        lua.create_function(|lua, (start_node, max_distance): (LuaTable, u32)| {
            // Extract start node data from Lua table
            let filepath: String = start_node.get("filepath")?;
            let _filename: String = start_node.get("filename")?;

            let base_dir = std::env::var("HOME").unwrap_or_default() + "/personal-wiki";

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
        })?,
    );

    Ok(exports)
}
