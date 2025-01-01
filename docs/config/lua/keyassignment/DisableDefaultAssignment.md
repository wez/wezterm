# `DisableDefaultAssignment`

Has no special meaning of its own; this action will undo the registration
of a default assignment if that key/mouse/modifier combination is one of the
default assignments and cause the key press to be propagated through
to the tab for processing.

```lua
config.keys = {
  -- Turn off the default CMD-m Hide action, allowing CMD-m to
  -- be potentially recognized and handled by the tab
  {
    key = 'm',
    mods = 'CMD',
    action = wezterm.action.DisableDefaultAssignment,
  },
}
```


