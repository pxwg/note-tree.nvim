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

---@param start_node BiDirectionalNode
---@param max_distance number
---@return table<string, BiDirectionalGraph>
function double_chain:find_all_related(start_node, max_distance)
  local rust_processor = require("utils.tree_builder").generate_double_chain_graph(start_node, max_distance)
  return rust_processor
end

---@param start_node BiDirectionalNode
---@param max number|nil
---@return BiDirectionalShortestPath[]
function double_chain:calculate_shortest_paths(start_node, max)
  max = max or math.huge
  local graph = self:find_all_related(start_node, max) or {}
  graph = graph or {}

  local shortest_paths = {}

  for _, data in pairs(graph) do
    for i = 1, #data do
      table.insert(shortest_paths, { node = data[i].links.filepath, path_length = data[i].distance })
    end
  end

  table.sort(shortest_paths, function(a, b)
    return a.path_length < b.path_length
  end)

  return shortest_paths
end

--- @param opt table
--- @param max number
local function show_buffer_inlines_menu(opt, max)
  require("util.note_telescope").double_chain_search(opt, max + 1)
end

M.show_buffer_inlines_menu = show_buffer_inlines_menu
M.double_chain = double_chain

return M
