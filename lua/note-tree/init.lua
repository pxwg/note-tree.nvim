local M = {}

function M.setup(opt)
  require("note-tree.commands")
  require("utils.tree_builder").initialize()
end

return M
