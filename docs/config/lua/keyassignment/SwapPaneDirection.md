# `SwapPaneDirection`

{{since('nightly')}}

Swaps the active pane with the adjacent pane in the specified direction. Focus
follows the pane that was active, so the same terminal remains active in its new
position. If no pane exists in that direction, this action does nothing.

Valid directions are `"Left"`, `"Right"`, `"Up"`, `"Down"`, `"Next"`, and
`"Prev"`. Directional ambiguity is resolved by the same rules as
[`ActivatePaneDirection`](ActivatePaneDirection.md).

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.keys = {
  { key = 'a', mods = 'ALT', action = act.SwapPaneDirection 'Left' },
  { key = 'd', mods = 'ALT', action = act.SwapPaneDirection 'Right' },
  { key = 'e', mods = 'ALT', action = act.SwapPaneDirection 'Up' },
  { key = 's', mods = 'ALT', action = act.SwapPaneDirection 'Down' },
}

return config
```

See also [PaneSelect](PaneSelect.md) for selecting a pane to swap and
[RotatePanes](RotatePanes.md) for rotating all panes.
