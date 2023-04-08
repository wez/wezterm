---
title: wezterm.action
tags:
 - keys
---

# `wezterm.action`

Helper for defining key assignment actions in your configuration file.
This is really just sugar for the underlying Lua -> Rust deserialation
mapping that makes it a bit easier to identify where syntax errors may
exist in your configuration file.

## Constructor Syntax

{{since('20220624-141144-bd1b7c5d')}}

`wezterm.action` is a special enum constructor type that makes it bit
more ergonomic to express the various actions than in earlier releases.
The older syntax is still supported, so you needn't scramble to update
your configuration files.

Indexing `wezterm.action` with a valid
[KeyAssignment](../keyassignment/index.md) name will act as a constructor for
that key assignment type.  For example, the lua expression:

```
wezterm.action.QuickSelectArgs
```

is a constructor for [QuickSelectArgs](../keyassignment/QuickSelectArgs.md).

If the key assignment type is a *unit variant* (has no parameters) such as
[Copy](../keyassignment/Copy.md), or can be constructed with default values
such as [QuickSelectArgs](../keyassignment/QuickSelectArgs.md) then you can
reference the constructor directly to have it evaluate as that value without
having to add any extra punctuation:

```lua
local wezterm = require 'wezterm'
return {
  keys = {
    {
      key = ' ',
      mods = 'CTRL|SHIFT',
      action = wezterm.action.QuickSelectArgs,
    },
  },
}
```

You may pass the optional parameters to `QuickSelectArgs` as you need
them, like this:

```lua
local wezterm = require 'wezterm'
return {
  keys = {
    {
      key = ' ',
      mods = 'CTRL|SHIFT',
      action = wezterm.action.QuickSelectArgs {
        alphabet = 'abc',
      },
    },
  },
}
```

If the key assignment type is a *tuple variant* (has positional parameters)
such as [ActivatePaneByIndex](../keyassignment/ActivatePaneByIndex.md), then
you can pass those by calling the constructor:

```lua
local wezterm = require 'wezterm'
-- shortcut to save typing below
local act = wezterm.action

return {
  keys = {
    { key = 'F1', mods = 'ALT', action = act.ActivatePaneByIndex(0) },
    { key = 'F2', mods = 'ALT', action = act.ActivatePaneByIndex(1) },
    { key = 'F3', mods = 'ALT', action = act.ActivatePaneByIndex(2) },
    { key = 'F4', mods = 'ALT', action = act.ActivatePaneByIndex(3) },
    { key = 'F5', mods = 'ALT', action = act.ActivatePaneByIndex(4) },
    { key = 'F6', mods = 'ALT', action = act.ActivatePaneByIndex(5) },
    { key = 'F7', mods = 'ALT', action = act.ActivatePaneByIndex(6) },
    { key = 'F8', mods = 'ALT', action = act.ActivatePaneByIndex(7) },
    { key = 'F9', mods = 'ALT', action = act.ActivatePaneByIndex(8) },
    { key = 'F10', mods = 'ALT', action = act.ActivatePaneByIndex(9) },

    -- Compare this with the older syntax shown in the section below
    { key = '{', mods = 'CTRL', action = act.ActivateTabRelative(-1) },
    { key = '}', mods = 'CTRL', action = act.ActivateTabRelative(1) },
  },
}
```

## Older versions

For versions before *20220624-141144-bd1b7c5d*, usage looks like this:

```lua
local wezterm = require 'wezterm'
return {
  keys = {
    {
      key = '{',
      mods = 'CTRL',
      action = wezterm.action {
        ActivateTabRelative = -1,
      },
    },
    {
      key = '}',
      mods = 'CTRL',
      action = wezterm.action {
        ActivateTabRelative = 1,
      },
    },
  },
}
```

The parameter is a lua representation of the underlying KeyAssignment enum from
the configuration code.  These docs aim to spell out sufficient examples that
you shouldn't need to learn to read Rust code, but there are occasions where
newly developed features are not yet documented and an enterprising user may
wish to go spelunking to figure them out!

[You can find the reference for available KeyAssignment values here](../keyassignment/index.md).
