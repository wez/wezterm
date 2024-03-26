# `tab:swap_active_pane_direction{ direction, keep_focus }`

{{since('nightly')}}

Swaps the active pane with the pane adjacent to it in the direction *direction*.
If *keep_focus* is true, focus is retained on the currently active pane but in its
new position.

Valid values for *direction* are:

* `"Left"`
* `"Right"`
* `"Up"`
* `"Down"`
* `"Prev"`
* `"Next"`

An example of usage is below:

```lua
local wezterm = require 'wezterm'
local config = {}

local function swap_active_pane_action(direction)
  return wezterm.action_callback(function(_window, pane)
    local tab = pane:tab()
    if tab ~= nil then
      tab:swap_active_pane_direction {
        direction = direction,
        keep_focus = true,
      }
    end
  end)
end

config.keys = {
  {
    key = 'LeftArrow',
    mods = 'CTRL|ALT',
    action = swap_active_pane_action 'Prev',
  },
  {
    key = 'RightArrow',
    mods = 'CTRL|ALT',
    action = swap_active_pane_action 'Next',
  },
}
return config
```

See [ActivatePaneDirection](../keyassignment/ActivatePaneDirection.md) for more information
about how panes are selected given a direction.
