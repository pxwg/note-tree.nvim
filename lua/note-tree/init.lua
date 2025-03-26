local M = {}

--- @type NoteTreeOpts
M.opts = { max_depth = 10, root_dir = vim.fn.expand("~/personal-wiki/") }

--- @class NoteTreeOpts
local default_opts = require("note-tree.defalut").default_opts

function M.setup(opt)
  M.opts = vim.tbl_deep_extend("force", default_opts, opt or {})
  require("note-tree.commands")
  require("utils.tree_builder").initialize()
end

return M
