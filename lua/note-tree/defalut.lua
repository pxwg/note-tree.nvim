local M = {}

--- @class NoteTreeOpts
--- @field root_dir string
--- @field max_depth number

--- Default options for the note-tree plugin
--- @type NoteTreeOpts
M.default_opts = {
  root_dir = vim.fn.expand("~/personal-wiki/"),
  max_depth = 10,
}

return M
