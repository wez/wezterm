# `SetWindowLevel`

{{since('nightly')}}
Set window level sepcified by the argument value. eg: `AlwaysOnTop` keeps the current window on top of other windows.

Accepted values: 

 * `"AlwaysOnBottom"`
 * `"Normal"` (this is the default)
 * `"AlwaysOnTop"`

```lua
config.keys = {
    {
        key = '[',
        mods = 'CMD',
        action = wezterm.action.SetWindowLevel("AlwaysOnBottom")
    }, 
    {
        key = '0',
        mods = 'CMD|SHIFT',
        action = wezterm.action.SetWindowLevel("Normal")
    }, 
    {
        key = ']',
        mods = 'CMD',
        action = wezterm.action.SetWindowLevel("AlwaysOnTop")
    }, 
}
```
