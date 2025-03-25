local M = {}

-- Module state to store loaded lib
local state = {
  tree_builder = nil,
  initialized = false,
}

-- Get the current script's directory to find the plugin root
local function get_plugin_root()
  local source = debug.getinfo(1, "S").source:sub(2)
  local script_dir = vim.fn.fnamemodify(source, ":h:h:h")
  return script_dir
end

local function try_load(path)
  local ok, result = pcall(package.loadlib, path, "luaopen_note_tree")
  if ok and type(result) == "function" then
    return result
  end
  return nil
end

local function initialize()
  if state.initialized then
    return state.tree_builder ~= nil
  end

  local plugin_root = get_plugin_root()
  local lib_path = plugin_root .. "/build/tree_builder_lua51"

  -- Try loading with different extensions based on the platform
  local lib_func = try_load(lib_path .. ".dylib") or try_load(lib_path .. ".so") or try_load(lib_path .. ".dll")

  if not lib_func then
    vim.notify("Failed to load tree_builder library. The library must export 'luaopen_note_tree'", vim.log.levels.ERROR)
    state.initialized = true
    return false
  end

  state.tree_builder = lib_func()
  state.initialized = true
  return true
end

local function ensure_loaded()
  if not state.initialized then
    return initialize()
  end
  return state.tree_builder ~= nil
end

--- Generate a double chain graph with lib tree_builder
---@param start_node BiDirectionalNode
---@param max_distance number
---@param base_dir string?
---@return BiDirectionalShortestPath[]
---@usage local graph = require("utils.tree_builder").generate_double_chain_graph(start_node, max_distance)
function M.generate_double_chain_graph(start_node, max_distance, base_dir)
  base_dir = base_dir or vim.fn.expand("~/personal-wiki")
  if not ensure_loaded() then
    return {}
  end
  return state.tree_builder.generate_double_chain_graph(start_node, max_distance, base_dir)
end

M.initialize = initialize
return M
