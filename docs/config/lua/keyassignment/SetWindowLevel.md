# `SetWindowLevel`

Set window level sepcified by the argument value. eg: `AlwaysOnTop` keeps the current window on top of other windows.

Accepted values: `AlwaysOnBottom` | `Normal` | `AlwaysOnTop`

```lua
local act = wezterm.action
local config = {}

config.keys = {
    {
        key = '[',
        mods = 'CMD',
        action = act.SetWindowLevel("AlwaysOnBottom")
    }, 
    {
        key = '0',
        mods = 'CMD|SHIFT',
        action = act.SetWindowLevel("Normal")
    }, 
    {
        key = ']',
        mods = 'CMD',
        action = act.SetWindowLevel("AlwaysOnTop")
    }, 
}
```
