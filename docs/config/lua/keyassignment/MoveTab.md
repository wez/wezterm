# `MoveTab`

Move the tab so that it has the index specified by the argument. eg: `0`
moves the tab to be  leftmost, while `1` moves the tab so that it is second tab
from the left, and so on.

```lua
local wezterm = require 'wezterm'
local config = {}

config.keys = {}

for i = 1, 8 do
  -- CTRL+ALT + number to move to that position
  table.insert(config.keys, {
    key = tostring(i),
    mods = 'CTRL|ALT',
    action = wezterm.action.MoveTab(i - 1),
  })
end

return config
```


