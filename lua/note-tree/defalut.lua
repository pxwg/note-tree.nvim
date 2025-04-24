local M = {}

--- @class NoteTreeOpts
--- @field root_dir string
--- @field log_path string
--- @field max_depth number

--- Default options for the note-tree plugin
--- @type NoteTreeOpts
M.default_opts = {
  root_dir = vim.fn.expand("~/personal-wiki/"),
  log_path = vim.fn.stdpath("data") .. "/note-tree.log",
  max_depth = 10,
}

return M
