# `SplitVertical`

{{since('20201031-154415-9614e117')}}

Splits the current pane in half vertically such that the current pane becomes
the top half and the new bottom half spawns a new command.

```lua
config.keys = {
  -- This will create a new split and run your default program inside it
  {
    key = '"',
    mods = 'CTRL|SHIFT|ALT',
    action = wezterm.action.SplitVertical { domain = 'CurrentPaneDomain' },
  },
}
```

`SplitVertical` requires a [SpawnCommand](../SpawnCommand.md) parameter to
specify what should be spawned into the new split.

```lua
config.keys = {
  -- This will create a new split and run the `top` program inside it
  {
    key = '"',
    mods = 'CTRL|SHIFT|ALT',
    action = wezterm.action.SplitVertical {
      args = { 'top' },
    },
  },
}
```

See also: [SplitPane](SplitPane.md).
