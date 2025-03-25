// A simple tool to generate a graph of markdown files in a wiki
// much faster than the original implementation: 2600ms ----> 400ms
// TODO: integration with neovim plugin
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

// Convert relative paths to absolute paths
fn convert_to_absolute_path(path: &str, base_dir: &str) -> String {
    if !path.starts_with("/") {
        format!("{}/{}", base_dir, path.replace("./", ""))
    } else {
        path.to_string()
    }
}

// Execute shell command and collect output
async fn execute_command_async(cmd: &str, args: &[&str]) -> String {
    let cmd = cmd.to_string();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    tokio::task::spawn_blocking(move || {
        Command::new(cmd)
            .args(&args)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

async fn get_forward_links_async(filepath: &str, base_dir: &str) -> Vec<String> {
    let pattern = "\\[.*?\\]\\((.*?\\.md)\\)";
    let args = vec!["-o", "--no-line-number", pattern, filepath];
    let output = execute_command_async("rg", &args).await;
    parse_links(&output, base_dir)
}

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

async fn generate_graph_async(
    start_file: &str,
    max_depth: u32,
    base_dir: &str,
) -> HashMap<String, Vec<String>> {
    let mut all_links = HashMap::new();
    let mut visited = HashSet::new();
    let mut current_layer = vec![Node {
        filepath: start_file.to_string(),
        distance: 0,
    }];
    visited.insert(start_file.to_string());
    println!("Starting breadth-first search on: {}", start_file);

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
                    .entry(target)
                    .or_insert_with(Vec::new)
                    .extend(sources);
            }

            for new_node in result.new_nodes {
                if !visited.contains(&new_node.filepath) {
                    visited.insert(new_node.filepath.clone());
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

async fn process_node_async(node: &Node, base_dir: &str) -> ProcessResult {
    let forward = get_forward_links_async(&node.filepath, base_dir).await;
    let backward = get_backward_links_async(&node.filepath, base_dir).await;

    let new_nodes = forward
        .iter()
        .chain(backward.iter())
        .map(|link| Node {
            filepath: link.clone(),
            distance: node.distance + 1,
        })
        .collect();

    let mut backward_links = HashMap::new();
    for bl in &backward {
        backward_links
            .entry(bl.clone())
            .or_insert_with(Vec::new)
            .push(node.filepath.clone());
    }

    ProcessResult {
        new_nodes,
        backward_links,
    }
}

fn print_dot_format(links: &HashMap<String, Vec<String>>) {
    println!("digraph wiki {{");
    println!("  node [shape=box];");

    for (target, sources) in links {
        let target_name = Path::new(target)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        for source in sources {
            let source_name = Path::new(source)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            println!("  \"{}\" -> \"{}\";", source_name, target_name);
        }
    }

    println!("}}");
}

#[mlua::lua_module]
fn note_tree(lua: &Lua) -> LuaResult<LuaTable> {
    let exports = lua.create_table()?;

    let _ = exports.set(
        "generate_double_chain_graph",
        lua.create_function(|lua, (start_node, max_distance): (LuaTable, u32)| {
            // Extract start node data from Lua table
            let filepath: String = start_node.get("filepath")?;
            let filename: String = start_node.get("filename")?;

            // Get base directory - modify this as needed
            let base_dir = std::env::var("HOME").unwrap_or_default() + "/personal-wiki";

            // Create and run a runtime for the async operations
            let rt = tokio::runtime::Runtime::new()?;
            let links = rt.block_on(generate_graph_async(&filepath, max_distance, &base_dir));

            // Convert the result to a Lua table
            let result_table = lua.create_table()?;

            for (target, sources) in links {
                let target_name = Path::new(&target)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                for source in sources {
                    let source_name = Path::new(&source)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    // Create a node entry for this link
                    let node_table = lua.create_table()?;
                    let node_info = lua.create_table()?;

                    node_info.set("filepath", source.clone())?;
                    node_info.set("filename", source_name)?;

                    node_table.set("node", node_info)?;
                    node_table.set("distance", 1)?; // We're setting a distance of 1 for each direct link

                    // Use target filename as the key
                    result_table.set(target_name.clone(), node_table)?;
                }
            }

            Ok(result_table)
        })?,
    );

    Ok(exports)
}
