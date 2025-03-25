local M = {}

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
---@return BiDirectionalShortestPath[]
function double_chain:calculate_shortest_paths(start_node, max)
  start_node = start_node or self
  max = max or math.huge
  local rust_processor = require("utils.tree_builder").generate_double_chain_graph(start_node, max)
  return rust_processor
end

--- @param opt table
--- @param max number
local function show_buffer_inlines_menu(opt, max)
  require("util.note_telescope").double_chain_search(opt, max + 1)
end

M.show_buffer_inlines_menu = show_buffer_inlines_menu
M.double_chain = double_chain

return M
