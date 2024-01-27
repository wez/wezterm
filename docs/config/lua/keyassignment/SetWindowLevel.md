# `SetWindowLevel`

{{since('20240127-113634-bbcac864')}}

Set window level specified by the argument value. eg: `AlwaysOnTop` keeps the current window on top of other windows.

Accepted values: 

 * `"AlwaysOnBottom"`
 * `"Normal"` (this is the default)
 * `"AlwaysOnTop"`

```lua
config.keys = {
  {
    key = '[',
    mods = 'CMD',
    action = wezterm.action.SetWindowLevel 'AlwaysOnBottom',
  },
  {
    key = '0',
    mods = 'CMD|SHIFT',
    action = wezterm.action.SetWindowLevel 'Normal',
  },
  {
    key = ']',
    mods = 'CMD',
    action = wezterm.action.SetWindowLevel 'AlwaysOnTop',
  },
}
```

!!! note 
    This functionality is currently only implemented on macOS. 
    The assigned values for window level will have no effect on other operating systems.
