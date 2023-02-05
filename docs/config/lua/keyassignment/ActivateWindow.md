# ActivateWindow(n)

*since: nightly builds only*

Activates the *nth* GUI window, zero-based.

Performing this action is equivalent to executing this lua code fragment:

```lua
wezterm.gui.gui_windows()[n + 1]:focus()
```

Here's an example of setting up hotkeys to activate specific windows:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

local mykeys = {}
for i = 1, 8 do
  -- CMD+ALT + number to activate that window
  table.insert(mykeys, {
    key = tostring(i),
    mods = 'CMD|ALT',
    action = act.ActivateWindow(i - 1),
  })
end

return {
  keys = mykeys,
}
```


See also 
[ActivateWindowRelative](ActivateWindowRelative.md),
[ActivateWindowRelativeNoWrap](ActivateWindowRelativeNoWrap.md).
