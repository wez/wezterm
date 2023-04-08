---
tags:
  - mouse
---

Mouse bindings are configurable, and there are a number of default assignments
described below.

The assignments are based around a triggering mouse event which may be combined
with a set of modifier keys to produce an action.

By default applications running in the terminal don't respond to the mouse.
However, applications can emit escape sequences to request mouse event tracking.
When mouse event tracking is enabled, mouse events are NOT matched against
the mouse assignments and are instead passed through to the application.

You can bypass the mouse reporting capture by holding down the `SHIFT` key;
that will prevent the event from being passed to the application and allow matching
it against your assignments as though the `SHIFT` key were not pressed.

The [bypass_mouse_reporting_modifiers](lua/config/bypass_mouse_reporting_modifiers.md)
option allows you to specify an alternative set of modifiers to use for
bypassing mouse reporting capture.

## Default Mouse Assignments

*Note: you can run `wezterm show-keys` to show the effective key and mouse assignments*.

In the table below, `Triple Left Down` means that the left mouse button is
being triple clicked and that the event matches the downstroke of the third
quick consecutive press.  `Triple Left Up` matches the subsequent release event
of that triple click, so for a triple click both
`SelectTextAtMouseCursor="Line"` and `CompleteSelection` will be triggered in
that order.

NOTE: In the action column, `act` is an alias to `wezterm.action` (to avoid repetition):

```lua
local act = wezterm.action
```

| Event | Modifiers | Action |
| --------- | --- | ------ |
| Triple Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Line")`  |
| Double Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Word")`  |
| Single Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Cell")`  |
| Single Left Down | `SHIFT`   | `act.ExtendSelectionToMouseCursor("Cell")`  |
| Single Left Down | `ALT`   | `act.SelectTextAtMouseCursor("Block")`  {{since('20220624-141144-bd1b7c5d', inline=True)}} |
| Single Left Up | `SHIFT`   | `act.CompleteSelectionOrOpenLinkAtMouseCursor("ClipboardAndPrimarySelection")`  |
| Single Left Up | `NONE`   | `act.CompleteSelectionOrOpenLinkAtMouseCursor("ClipboardAndPrimarySelection")`  |
| Single Left Up | `ALT`   | `act.CompleteSelection("ClipboardAndPrimarySelection")`  {{since('20220624-141144-bd1b7c5d', inline=True)}} |
| Double Left Up | `NONE`   | `act.CompleteSelection("ClipboardAndPrimarySelection")`  |
| Triple Left Up | `NONE`   | `act.CompleteSelection("ClipboardAndPrimarySelection")`  |
| Single Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Cell")`  |
| Single Left Drag | `ALT`   | `act.ExtendSelectionToMouseCursor("Block")` {{since('20220624-141144-bd1b7c5d', inline=True)}} |
| Single Left Down | `ALT+SHIFT`   | `act.ExtendSelectionToMouseCursor("Block")`  {{since('20220624-141144-bd1b7c5d', inline=True)}} |
| Single Left Up | `ALT+SHIFT`   | `act.CompleteSelection("ClipboardAndPrimarySelection")`  {{since('20220624-141144-bd1b7c5d', inline=True)}} |
| Double Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Word")`  |
| Triple Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Line")`  |
| Single Middle Down | `NONE`   | `act.PasteFrom("PrimarySelection")`  |
| Single Left Drag | `SUPER` | `act.StartWindowDrag` (*since 20210314-114017-04b7cedd*) |
| Single Left Drag | `CTRL+SHIFT` | `act.StartWindowDrag` (*since 20210314-114017-04b7cedd*) |

If you don't want the default assignments to be registered, you can
disable all of them with this configuration; if you chose to do this,
you must explicitly register every binding.

```lua
config.disable_default_mouse_bindings = true
```

## Configuring Mouse Assignments

{{since('20200607-144723-74889cd4')}}

You can define mouse actions using the `mouse_bindings` configuration section:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.mouse_bindings = {
  -- Right click sends "woot" to the terminal
  {
    event = { Down = { streak = 1, button = 'Right' } },
    mods = 'NONE',
    action = act.SendString 'woot',
  },

  -- Change the default click behavior so that it only selects
  -- text and doesn't open hyperlinks
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'NONE',
    action = act.CompleteSelection 'ClipboardAndPrimarySelection',
  },

  -- and make CTRL-Click open hyperlinks
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'CTRL',
    action = act.OpenLinkAtMouseCursor,
  },
  -- NOTE that binding only the 'Up' event can give unexpected behaviors.
  -- Read more below on the gotcha of binding an 'Up' event only.
}

return config
```

Each entry in the mouse binding table can have the following fields:

* `event` - the mouse event on which to trigger. Described in detail below.
* `mods` - the keyboard modifier keys that must be active in order to match the event.
  `mods` have the same definition and meaning as for key assignments and are described
  in more detail in [Configuring Key Assignments](keys.md#configuring-key-assignments).
* `action` - the action to take when this mouse binding is matched
* `mouse_reporting` - an optional boolean that defaults to `false`. This mouse binding
   entry will only be considered if the current pane's mouse reporting state matches.
   In general, you should avoid defining assignments that have
   `mouse_reporting=true` as it will prevent the application running in the
   pane from receiving that mouse event.  You can, of course, define these and
   still send your mouse event to the pane by holding down the configured
   [mouse reporting bypass modifier
   key](lua/config/bypass_mouse_reporting_modifiers.md). {{since('20220807-113146-c2fee766', inline=True)}}
* `alt_screen` - an optional field that defaults to `'Any'`, but that can also
  be set to either `true` or `false`. This mouse binding entry will only be
  considered if the current pane's alt screen state matches this field.  Most
  of the default mouse assignments are defined as `alt_screen='Any'`, a notable
  exception being that mouse wheel scrolling only applies when
  `alt_screen=false`, as the mouse wheel is typically mapped to arrow keys by
  the terminal in alt screen mode. {{since('20220807-113146-c2fee766', inline=True)}}.

The `action` and `mods` portions are described in more detail in the key assignment
information below.

The `event` portion has three components:

* Whether it is a `Down`, `Up` or `Drag` event
* The number of consecutive clicks within the click threshold (the *click streak*)
* The mouse button; `Left`, `Right`, or `Middle`.

A double click is a `down-up-down` sequence where either the second button down
is held for long enough or is released and no subsequent down event occurs
within the click threshold.  When recognized, it emits a `Down` event with
`streak=2`.  If the mouse is moved while the button is held, a `Drag` event
with `streak=2` is generated.  When the mouse button is released an `Up` event
with `streak=2` is generated.

The mouse event recognizer supports an arbitrary click streak, so if
you wanted quadruple-click bindings you can specify `streak=4`.

| Event             | Lua Representation  |
| ----------------- | ------------------- |
| Triple Left Down  | `event={Down={streak=3, button="Left"}}` |
| Double Left Up  | `event={Up={streak=2, button="Left"}}` |
| Single Left Drag  | `event={Drag={streak=1, button="Left"}}` |

{{since('20220807-113146-c2fee766')}}

You can handle vertical wheel scroll events using the example shown below. The
`streak` and amount associated with either `WheelUp` or `WheelDown` are set to
`1` for the sake of simplicity of matching the event; you may use
[`window:current_event`](lua/window/current_event.md), if to access the actual
delta scroll value while handling the event.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.mouse_bindings = {
  -- Scrolling up while holding CTRL increases the font size
  {
    event = { Down = { streak = 1, button = { WheelUp = 1 } } },
    mods = 'CTRL',
    action = act.IncreaseFontSize,
  },

  -- Scrolling down while holding CTRL decreases the font size
  {
    event = { Down = { streak = 1, button = { WheelDown = 1 } } },
    mods = 'CTRL',
    action = act.DecreaseFontSize,
  },
}

return config
```


# Gotcha on binding an 'Up' event only

If you only have a mouse bind on the 'Up' event and not on the 'Down' event,
the 'Down' event will still be sent to the running program.
If that program is tracking mouse inputs (like tmux or vim with mouse support),
you may experience _unintuitive behavior_ as the program receives the 'Down'
event, but not the 'Up' event (which is bound to something in your config).

To avoid this, it is recommended to disable the 'Down' event (to ensure it won't
be sent to the running program), for example:
```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

config.mouse_bindings = {
  -- Bind 'Up' event of CTRL-Click to open hyperlinks
  {
    event = { Up = { streak = 1, button = 'Left' } },
    mods = 'CTRL',
    action = act.OpenLinkAtMouseCursor,
  },
  -- Disable the 'Down' event of CTRL-Click to avoid weird program behaviors
  {
    event = { Down = { streak = 1, button = 'Left' } },
    mods = 'CTRL',
    action = act.Nop,
  },
}
return config
```


# Available Actions

See the [`KeyAssignment` reference](lua/keyassignment/index.md) for information
on available actions.
