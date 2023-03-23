# `SplitHorizontal`

{{since('20201031-154415-9614e117')}}

Splits the current pane in half horizontally such that the current pane becomes
the left half and the new right half spawns a new command.

```lua
config.keys = {
  -- This will create a new split and run your default program inside it
  {
    key = '%',
    mods = 'CTRL|SHIFT|ALT',
    action = wezterm.action.SplitHorizontal { domain = 'CurrentPaneDomain' },
  },
}
```

`SplitHorizontal` requires a [SpawnCommand](../SpawnCommand.md) parameter to
specify what should be spawned into the new split.

```lua
config.keys = {
  -- This will create a new split and run the `top` program inside it
  {
    key = '%',
    mods = 'CTRL|SHIFT|ALT',
    action = wezterm.action.SplitHorizontal {
      args = { 'top' },
    },
  },
}
```

See also: [SplitPane](SplitPane.md).
