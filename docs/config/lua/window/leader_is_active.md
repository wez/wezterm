# wezterm:leader_is_active()

*Since: nightly builds only*

Returns `true` if the [Leader Key](../../keys.md) is active in the window, or false otherwise.

This example shows `LEADER` in the right status area, and turns the cursor orange,
when the leader is active:

```lua
local wezterm = require 'wezterm';

wezterm.on("update-right-status", function(window, pane)
  local leader = ""
  if window:leader_is_active() then
    leader = "LEADER"
  end
  window:set_right_status(leader)
end);

return {
  leader = { key="a", mods="CTRL" },
  colors = {
    dead_key_cursor = "orange",
  },
}
```

See also: [window:dead_key_is_active()](dead_key_is_active.md).
