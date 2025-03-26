local M = {}
local note_tree = require("note-tree")

--- @class BiDirectionalNode
--- @field filepath string
--- @field filename string

--- @class BiDirectionalNode
local double_chain = {}

--- @class BiDirectionalGraph
--- @field node BiDirectionalNode
--- @field distance number

---@class BiDirectionalShortestPath
---@field node string
---@field path_length number

---@param start_node BiDirectionalNode?
---@param max number?
---@param base_dir string?
---@return BiDirectionalShortestPath[]
function double_chain:get_nodes(start_node, max, base_dir)
  base_dir = base_dir or vim.tbl_extend("force", {}, note_tree.opts).root
  start_node = start_node or self
  max = max or 10
  local rust_processor = require("utils.tree_builder").generate_double_chain_graph(start_node, max, base_dir)
  return rust_processor
end

M.double_chain = double_chain

return M
