local M = {}

-- Get the current script's directory to find the plugin root
local function get_plugin_root()
  local source = debug.getinfo(1, "S").source:sub(2)
  local script_dir = vim.fn.fnamemodify(source, ":h:h:h")
  return script_dir
end

local plugin_root = get_plugin_root()
local lib_path = plugin_root .. "/build/tree_builder_lua51"

local function try_load(path)
  local ok, result = pcall(package.loadlib, path, "luaopen_note_tree")
  if ok and type(result) == "function" then
    return result
  end
  return nil
end

-- Try loading with different extensions
-- TODO: Load the correct extension based on the platform
local lib_func = try_load(lib_path .. ".dylib") or try_load(lib_path .. ".so") or try_load(lib_path .. ".dll")

if not lib_func then
  vim.notify("Failed to load tree_builder library. The library must export 'luaopen_note_tree'", vim.log.levels.ERROR)
end

if not lib_func then
  return
end
local tree_builder = lib_func()

for k, v in pairs(tree_builder) do
  M[k] = v
end

--- Generate a double chain graph with lib tree_builder
--- @param start_node BiDirectionalNode
--- @param max_distance number
--- @return table<string, BiDirectionalGraph>
function M.generate_double_chain_graph(start_node, max_distance)
  return tree_builder.generate_double_chain_graph(start_node, max_distance)
end

return M
