--- TODO: add customizable base directory
local usrcmd = vim.api.nvim_create_user_command

--- search and jump to the node via local tree
usrcmd("NoteTreeLocal", function(opts)
  local start_time = vim.uv.hrtime()

  local number = tonumber(opts.fargs[1]) or 10
  local local_node = { filepath = vim.fn.expand("%:p"), filename = vim.fn.expand("%:t:r") }
  local base_dir = vim.fn.expand("~/personal-wiki")
  require("integrate.telescope").double_chain_search({}, local_node, number + 1, base_dir)

  local end_time = vim.loop.hrtime()

  vim.api.nvim_echo({ { "  Build tree in: " .. ((end_time - start_time) / 1e6) .. " ms" } }, false, {})
end, { nargs = "?" })

--- Insert a new link into the wiki file
usrcmd("NoteTreeLocalInsert", function(opts)
  local start_time = vim.uv.hrtime()

  local number = tonumber(opts.fargs[1]) or 10
  local local_node = { filepath = vim.fn.expand("%:p"), filename = vim.fn.expand("%:t:r") }
  local base_dir = vim.fn.expand("~/personal-wiki")
  require("integrate.telescope").double_chain_insert({}, local_node, number + 1, base_dir)

  local end_time = vim.loop.hrtime()

  vim.api.nvim_echo({ { "  Build tree in: " .. ((end_time - start_time) / 1e6) .. " ms" } }, false, {})
end, { nargs = "?" })

--- search and jump to the node via global tree
usrcmd("NoteTreeGlobal", function(opts)
  local start_time = vim.uv.hrtime()

  local number = tonumber(opts.fargs[1]) or 10
  local global_node = { filepath = vim.fn.expand("~/personal-wiki/index.md"), filename = "index" }
  local base_dir = vim.fn.expand("~/personal-wiki")
  require("integrate.telescope").double_chain_search({}, global_node, number + 1, base_dir)

  local end_time = vim.loop.hrtime()

  vim.api.nvim_echo({ { "  Build tree in: " .. ((end_time - start_time) / 1e6) .. " ms" } }, false, {})
end, { nargs = "?" })
