# window:leader_is_active()

{{since('20220319-142410-0fcdea07')}}

Returns `true` if the [Leader Key](../../keys.md) is active in the window, or false otherwise.

This example shows `LEADER` in the right status area, and turns the cursor orange,
when the leader is active:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-right-status', function(window, pane)
  local leader = ''
  if window:leader_is_active() then
    leader = 'LEADER'
  end
  window:set_right_status(leader)
end)

return {
  leader = { key = 'a', mods = 'CTRL' },
  colors = {
    compose_cursor = 'orange',
  },
}
```

See also: [window:composition_status()](composition_status.md).
