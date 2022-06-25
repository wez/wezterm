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

In the table below, `Triple Left Down` means that the left mouse button is
being triple clicked and that the event matches the downstroke of the third
quick consecutive press.  `Triple Left Up` matches the subsequent release event
of that triple click, so for a triple click both
`SelectTextAtMouseCursor="Line"` and `CompleteSelection` will be triggered in
that order.

NOTE: In the action column, `act` is an alias to `wezterm.action` (to avoid repetition).

| Event | Modifiers | Action |
| --------- | --- | ------ |
| Triple Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Line")`  |
| Double Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Word")`  |
| Single Left Down | `NONE`   | `act.SelectTextAtMouseCursor("Cell")`  |
| Single Left Down | `SHIFT`   | `act.ExtendSelectionToMouseCursor("Cell")`  |
| Single Left Down | `ALT`   | `act.SelectTextAtMouseCursor("Block")`  (*since: 20220624-141144-bd1b7c5d*) |
| Single Left Up | `SHIFT`   | `act.CompleteSelectionOrOpenLinkAtMouseCursor("PrimarySelection")`  |
| Single Left Up | `NONE`   | `act.CompleteSelectionOrOpenLinkAtMouseCursor("PrimarySelection")`  |
| Single Left Up | `ALT`   | `act.CompleteSelection("PrimarySelection")`  (*since: 20220624-141144-bd1b7c5d*) |
| Double Left Up | `NONE`   | `act.CompleteSelection("PrimarySelection")`  |
| Triple Left Up | `NONE`   | `act.CompleteSelection("PrimarySelection")`  |
| Single Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Cell")`  |
| Single Left Drag | `ALT`   | `act.ExtendSelectionToMouseCursor("Block")` (*since: 20220624-141144-bd1b7c5d*) |
| Single Left Down | `ALT+SHIFT`   | `act.ExtendSelectionToMouseCursor("Block")`  (*since: 20220624-141144-bd1b7c5d*) |
| Single Left Up | `ALT+SHIFT`   | `act.CompleteSelection("PrimarySelection")`  (*since: 20220624-141144-bd1b7c5d*) |
| Double Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Word")`  |
| Triple Left Drag | `NONE`   | `act.ExtendSelectionToMouseCursor("Line")`  |
| Single Middle Down | `NONE`   | `act.PasteFrom("PrimarySelection")`  |
| Single Left Drag | `SUPER` | `act.StartWindowDrag` (*since 20210314-114017-04b7cedd*) |
| Single Left Drag | `CTRL+SHIFT` | `act.StartWindowDrag` (*since 20210314-114017-04b7cedd*) |

If you don't want the default assignments to be registered, you can
disable all of them with this configuration; if you chose to do this,
you must explicitly register every binding.

```lua
return {
  disable_default_mouse_bindings = true,
}
```

## Configuring Mouse Assignments

*since: 20200607-144723-74889cd4*

You can define mouse actions using the `mouse_bindings` configuration section:

```lua
local wezterm = require 'wezterm';
local act = wezterm.action

return {
  mouse_bindings = {
    -- Right click sends "woot" to the terminal
    {
      event={Down={streak=1, button="Right"}},
      mods="NONE",
      action=act.SendString("woot"),
    },

    -- Change the default click behavior so that it only selects
    -- text and doesn't open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="NONE",
      action=act.CompleteSelection("PrimarySelection"),
    },

    -- and make CTRL-Click open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="CTRL",
      action=act.OpenLinkAtMouseCursor,
    },
    -- NOTE that binding only the 'Up' event can give unexpected behaviors.
    -- Read more below on the gotcha of binding an 'Up' event only.
  },
}
```

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


# Gotcha on binding an 'Up' event only

If you only have a mouse bind on the 'Up' event and not on the 'Down' event,
the 'Down' event will still be sent to the running program.
If that program is tracking mouse inputs (like tmux or vim with mouse support),
you may experience _unintuitive behavior_ as the program receives the 'Down'
event, but not the 'Up' event (which is bound to something in your config).

To avoid this, it is recommended to disable the 'Down' event (to ensure it won't
be sent to the running program), for example:
```lua
local wezterm = require "wezterm"
local act = wezterm.action

return {
  mouse_bindings = {
    -- Bind 'Up' event of CTRL-Click to open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="CTRL",
      action=act.OpenLinkAtMouseCursor,
    },
    -- Disable the 'Down' event of CTRL-Click to avoid weird program behaviors
    {
      event={Down={streak=1, button="Left"}},
      mods="CTRL",
      action=act.Nop,
    },
  },
}
```


# Available Actions

See the [`KeyAssignment` reference](lua/keyassignment/index.md) for information
on available actions.
