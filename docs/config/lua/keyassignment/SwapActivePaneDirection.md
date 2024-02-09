# `SwapActivePaneDirection`

{{since('nightly')}}

`SwapActivePaneDirection` swaps the active pane with the pane adjacent to it in
a specific direction.

See [ActivatePaneDirection](../keyassignment/ActivatePaneDirection.md) for more information
about how panes are selected given a direction.

The action requires two named arguments, *direction* and *keep_focus*.

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

config.keys = {
  {
    key = 'LeftArrow',
    mods = 'CTRL|ALT',
    action = {
      SwapActivePaneDirection = { direction = 'Prev', keep_focus = true },
    },
  },
  {
    key = 'RightArrow',
    mods = 'CTRL|ALT',
    action = {
      SwapActivePaneDirection = { direction = 'Next', keep_focus = true },
    },
  },
}
return config
```
