# `wezterm.permute_any_or_no_mods(table)`

*Since: nightly builds only*

This function is intended to help with generating key or mouse binding
entries that should apply regardless of the combination of modifier keys
pressed.

For each combination of modifiers `CTRL`, `ALT`, `SHIFT` and `SUPER`,
the supplied table value is copied and has `mods = <value>` set into
the copy.

In addition, an entry for `NONE` *is* generated (this is the only
difference between `permute_any_mods` and `permute_any_or_no_mods`).

An array holding all of those combinations is returned.

The array can be unpacked into a lua table initializer and used like this:

```lua
local wezterm = require 'wezterm';

return {
  mouse_bindings = {
    table.unpack(wezterm.permute_any_mods({
      event={Down={streak=1, button="Middle"}},
      action="PastePrimarySelection"
    }))
  }
}
```

This is equivalent to writing this out, but is much less verbose:

```lua
return {
  mouse_bindings = {
        {
            action= "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "NONE",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "ALT",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "ALT | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | ALT",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | ALT | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "CTRL",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "CTRL | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "ALT | CTRL",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "ALT | CTRL | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | CTRL",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | CTRL | SUPER",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | ALT | CTRL",
        },
        {
            action = "PastePrimarySelection",
            event = {
                Down = {
                    button = "Middle",
                    streak = 1,
                },
            },
            mods = "SHIFT | ALT | CTRL | SUPER",
        },
  }
}
```
