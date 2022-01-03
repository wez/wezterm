# wezterm:dead_key_is_active()

*Since: nightly builds only*

Returns `true` if a dead key is active in the window, or false otherwise.

This example shows `DEAD` in the right status area, and turns the cursor orange,
when a dead key is active:

```lua
local wezterm = require 'wezterm';

wezterm.on("update-right-status", function(window, pane)
  local dead = ""
  if window:dead_key_is_active() then
    dead = "DEAD"
  end
  window:set_right_status(dead)
end);

return {
  leader = { key="a", mods="CTRL" },
  colors = {
    dead_key_cursor = "orange",
  },
}
```

See also: [window:leader_is_active()](leader_is_active.md).

