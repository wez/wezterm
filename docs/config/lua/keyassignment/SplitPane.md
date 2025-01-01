# `SplitPane`

{{since('20220624-141144-bd1b7c5d')}}

Splits the active pane in a particular direction, spawning a new command into the newly created pane.

This assignment has a number of fields that control the overall action:

* `direction` - can be one of `"Up"`, `"Down"`, `"Left"`, `"Right"`. Specifies where the new pane will end up. This field is required.
* `size` - controls the size of the new pane. Can be `{Cells=10}` to specify eg: 10 cells or `{Percent=50}` to specify 50% of the available space.  If omitted, `{Percent=50}` is the default
* `command` - the [SpawnCommand](../SpawnCommand.md) that specifies what program to launch into the new pane. If omitted, the [default_prog](../config/default_prog.md) is used
* `top_level` - if set to `true`, rather than splitting the active pane, the split will be made at the root of the tab and effectively split the entire tab across the full extent possible.  The default is `false`.

```lua
config.keys = {
  -- This will create a new split and run the `top` program inside it
  {
    key = '%',
    mods = 'CTRL|SHIFT|ALT',
    action = wezterm.action.SplitPane {
      direction = 'Left',
      command = { args = { 'top' } },
      size = { Percent = 50 },
    },
  },
}
```

See also: [SplitHorizontal](SplitHorizontal.md), [SplitVertical](SplitVertical.md) and `wezterm cli split-pane --help`.
