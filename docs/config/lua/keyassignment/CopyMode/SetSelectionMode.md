# CopyMode { SetSelectionMode = MODE }

*Since: 20220624-141144-bd1b7c5d*

Sets the CopyMode selection mode.

MODE can be one of:

* `"Cell"` - selection expands a single cell at a time
* `"Word"` - selection expands by a word at a time
* `"Line"` - selection expands by a line at a time
* `"Block"` - selection expands to define a rectangular block using the starting point and current cursor position as the corners
* `"SemanticZone"` - selection expands to the current semantic zone. See [Shell Integration](../../../../shell-integration.md). (*Since: 20220903-194523-3bb1ed61*).

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      {
        key = 'v',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Cell' },
      },
    },
  },
}
```

See also: [ClearSelectionMode](ClearSelectionMode.md).
