local M = {}

local lib_path = vim.fn.expand("~/note-tree.nvim/build/tree_builder_lua51.dylib")

local function try_load(path)
  -- print("Trying to load", path)
  local ok, result = pcall(package.loadlib, path, "luaopen_note_tree")
  -- print("  Result:", ok, type(result))
  if ok and type(result) == "function" then
    return result
  end
  return nil
end

-- Try loading with different extensions and paths
local lib_func = try_load(lib_path .. ".dylib")
  or try_load(lib_path .. ".so")
  or try_load(vim.fn.expand("~/note-tree.nvim/build/tree_builder_lua51.dylib"))

if not lib_func then
  error("Failed to load tree_builder library. The library must export 'luaopen_note_tree'")
end

-- Call the loader function to get the module table
local tree_builder = lib_func()

for k, v in pairs(tree_builder) do
  M[k] = v
end

return M
