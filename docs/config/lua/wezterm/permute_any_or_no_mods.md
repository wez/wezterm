---
title: wezterm.permute_any_or_no_mods
tags:
 - utility
 - keys
---
# `wezterm.permute_any_or_no_mods(table)`

{{since('20201031-154415-9614e117')}}

This function is intended to help with generating key or mouse binding
entries that should apply regardless of the combination of modifier keys
pressed.

For each combination of modifiers `CTRL`, `ALT`, `SHIFT` and `SUPER`,
the supplied table value is copied and has `mods = <value>` set into
the copy.

In addition, an entry for `NONE` *is* generated (this is the only
difference between `permute_any_mods` and `permute_any_or_no_mods`).

An array holding all of those combinations is returned.

If this is your only binding, or it is the _last_ binding, the resulting array can be unpacked into a lua table initializer and used like this:

```lua
local wezterm = require 'wezterm'

return {
  mouse_bindings = {
    table.unpack(wezterm.permute_any_or_no_mods {
      event = { Down = { streak = 1, button = 'Middle' } },
      action = 'PastePrimarySelection',
    }),
  },
}
```
(and if you have other bindings before and/or after, use a for loop to iterate and add each binding to your bindings table)

This is equivalent to writing this out, but is much less verbose:

```lua
return {
  mouse_bindings = {
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'NONE',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'ALT',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'ALT | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | ALT',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | ALT | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'CTRL',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'CTRL | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'ALT | CTRL',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'ALT | CTRL | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | CTRL',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | CTRL | SUPER',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | ALT | CTRL',
    },
    {
      action = 'PastePrimarySelection',
      event = {
        Down = {
          button = 'Middle',
          streak = 1,
        },
      },
      mods = 'SHIFT | ALT | CTRL | SUPER',
    },
  },
}
```
