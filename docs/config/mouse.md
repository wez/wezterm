Mouse bindings are configurable.

The assignments are based around a triggering mouse event which may be combined
with a set of modifier keys to produce an action.

## Default Mouse Assignments

In the table below, `Triple Left Down` means that the left mouse button is
being triple clicked and that the event matches the downstroke of the third
quick consecutive press.  `Triple Left Up` matches the subsequent release event
of that triple click, so for a triple click both
`SelectTextAtMouseCursor="Line"` and `CompleteSelection` will be triggered in
that order.

| Event | Modifiers | Action |
| --------- | --- | ------ |
| Triple Left Down | `NONE`   | `SelectTextAtMouseCursor="Line"`  |
| Double Left Down | `NONE`   | `SelectTextAtMouseCursor="Word"`  |
| Single Left Down | `NONE`   | `SelectTextAtMouseCursor="Cell"`  |
| Single Left Down | `SHIFT`   | `ExtendSelectionToMouseCursor={}`  |
| Single Left Up | `NONE`   | `CompleteSelectionOrOpenLinkAtMouseCursor="PrimarySelection"`  |
| Double Left Up | `NONE`   | `CompleteSelection="PrimarySelection"`  |
| Triple Left Up | `NONE`   | `CompleteSelection="PrimarySelection"`  |
| Single Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Cell"`  |
| Double Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Word"`  |
| Triple Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Line"`  |
| Single Middle Down | `NONE`   | `PasteFrom="PrimarySelection"`  |
| Single Left Drag | `SUPER` | `StartWindowDrag` (*since 20210314-114017-04b7cedd*) |
| Single Left Drag | `CTRL+SHIFT` | `StartWindowDrag` (*since 20210314-114017-04b7cedd*) |

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

return {
  mouse_bindings = {
    -- Right click sends "woot" to the terminal
    {
      event={Down={streak=1, button="Right"}},
      mods="NONE",
      action=wezterm.action{SendString="woot"}
    },

    -- Change the default click behavior so that it only selects
    -- text and doesn't open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="NONE",
      action=wezterm.action{CompleteSelection="PrimarySelection"},
    },

    -- and make CTRL-Click open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="CTRL",
      action="OpenLinkAtMouseCursor",
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
return {
  mouse_bindings = {
    -- Bind 'Up' event of CTRL-Click to open hyperlinks
    {
      event={Up={streak=1, button="Left"}},
      mods="CTRL",
      action="OpenLinkAtMouseCursor",
    },
    -- Disable the 'Down' event of CTRL-Click to avoid weird program behaviors
    {
      event={Down={streak=1, button="Left"}},
      mods="CTRL",
      action="Nop",
    },
  },
}
```


# Available Actions

See the [`KeyAssignment` reference](lua/keyassignment/index.md) for information
on available actions.


