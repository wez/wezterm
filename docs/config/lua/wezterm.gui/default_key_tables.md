# `wezterm.gui.default_key_tables()`

{{since('20221119-145034-49b9839f')}}

Returns a table holding the effective default set of `key_tables`.  That is the
set of keys that is used as a base if there was no configuration file.

This is useful in cases where you want to override a key table assignment
without replacing the entire set of key tables.

This example shows how to add a key assignment for `Backspace` to `copy_mode`,
without having to manually specify the entire key table:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

local copy_mode = nil
if wezterm.gui then
  copy_mode = wezterm.gui.default_key_tables().copy_mode
  table.insert(
    copy_mode,
    { key = 'Backspace', mods = 'NONE', action = act.CopyMode 'MoveLeft' }
  )
end

return {
  key_tables = {
    copy_mode = copy_mode,
  },
}
```
