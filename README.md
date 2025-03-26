# note-tree.nvim 🌳

Buiding a tree of notes in Neovim.

## Introduction

The structure of the knowledge is a connected diagrams, which could abstractly be represented as a tree. This plugin is designed to help you build a tree of notes in Neovim.

## Installation

```lua
return {
  "pxwg/note-tree.nvim",
  event = "VeryLazy",
  build = "make lua51",
  opts = {
    max_depth = 10,
    root = "~/personal-wiki",
  },
}
```
