Both the keyboard and the mouse bindings are configurable.

The assignments are based around a triggering event, such as a keypress or
mouse button click, which is combined with a set of modifier keys to produce
an action.

A full list of possible keys, mouse events and actions are included below,
after these tables describing the default assignments.

## Alt / Option Key Behavior & Composed Keys

The operating system has its own user selectable keymap that is sometimes at
odds with old-school terminal emulation that pre-dates internationalization as
a concept.  WezTerm tries to behave reasonably by default, but also give you
control in other situations.

### Layouts with an AltGr key

If you have, for example, a European keyboard layout with an AltGr key then
wezterm will respect the composition effects of AltGr produced by the system.
For example, in a German keymap, `AltGr <` will produce `|`.

If your physical keyboard doesn't match the keyboard layout (eg: using a US
keyboard with DEU selected in the OS), then the right hand `Alt` key is often
re-interpreted as having the `AltGr` function with behavior as described above.

The left `Alt` will be treated as a modifier with no composition effects.

### macOS Left and Right Option Key

*since: 20200620-160318-e00b076c*

The default behavior is to treat the left `Option` key as the `Alt` modifier
with no composition effects, while the right `Option` key performs composition
(making it approximately equivalent to `AltGr` on other operating systems).

You can control this behavior in your configuration:

```lua
return {
  send_composed_key_when_left_alt_is_pressed=false,
  send_composed_key_when_right_alt_is_pressed=true,
}
```

If you're running an earlier release the options were a bit more limited;
both left and right `Option` keys behave identically and composition
behavior was influenced for both of them via the `send_composed_key_when_alt_is_pressed`
configuration option.

*since: 20210203-095643-70a364eb*

WezTerm is now able to perform dead-key expansion when `use_ime = false`.  Dead
keys are treated as composition effects, so with the default settings of
`send_composed_key_when_left_alt_is_pressed` and
`send_composed_key_when_right_alt_is_pressed` above, in a US layout, `Left-Opt
n` will produce `Alt N` and `Right-Opt n` will will for a subsequent key press
before generating an event; `Right-Opt n SPACE` will emit `~` whereas `Right-Opt n
n` will emit `ñ`.

You may also set `use_dead_keys = false` to skip the hold state; continuing
the example above, `Right-Opt n` will then immediately produce `~`.

### macOS and the Input Method Editor (IME)

WezTerm has support for using the operating system Input Method Editor (IME) on
macOS.  This is useful in cases where you need to type kanji or are using a
keyboard layout with dead keys.  However, the input method editor can get in
the way and has a couple of irritating side effects such as preventing key
repeat for a subset of keys.

You can control whether the IME is enabled on macOS in your configuration file:

```lua
return {
  use_ime = false,
}
```

*since: 20200620-160318-e00b076c*

The default for `use_ime` is false.  The default in earlier releases was `true`.

### Microsoft Windows and Dead Keys

*since: 20201031-154415-9614e117*

By default, if you are using a layout with *dead keys* (eg: US International
layout, or a number of European layouts such as German or French) pressing
a dead key in wezterm will "hold" the dead key until the next character is
pressed, resulting in a combined character with a diacritic.  For example,
pressing `^` and then `e` will produce `ê`.  Pressing `^` then `SPACE`
will produce `^` on its own.

If you are a heavy user of Vi style editors then you may wish to disable
dead key processing so that `^` can be used with a single keypress.

You can tell WezTerm to disable dead keys by setting this in your configuration
file:

```lua
return {
  use_dead_keys = false
}
```

### Defining Assignments for key combinations that may be composed

When a key combination produces a composed key result, wezterm will look up
both the composed and uncomposed versions of the keypress in your key mappings.
If either lookup matches your assignment, that will take precedence over
the normal key processing.

## Default Shortcut / Key Binding Assignments

The default key bindings are:

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `c`   | `CopyTo="Clipboard"`  |
| `SUPER`     | `v`   | `PasteFrom="Clipboard"`  |
| `CTRL+SHIFT`     | `c`   | `CopyTo="Clipboard"`  |
| `CTRL+SHIFT`     | `v`   | `PasteFrom="Clipboard"`  |
| `CTRL`     | `Insert` | `CopyTo="PrimarySelection"` (*since: 20210203-095643-70a364eb*) |
| `SHIFT`     | `Insert` | `PasteFrom="PrimarySelection"` |
| `SUPER`     | `m`      | `Hide`  |
| `SUPER`     | `n`      | `SpawnWindow` |
| `CTRL+SHIFT`     | `n`      | `SpawnWindow` |
| `ALT`       | `Enter`  | `ToggleFullScreen` |
| `SUPER`     | `-`      | `DecreaseFontSize` |
| `CTRL`      | `-`      | `DecreaseFontSize` |
| `SUPER`     | `=`      | `IncreaseFontSize` |
| `CTRL`      | `=`      | `IncreaseFontSize` |
| `SUPER`     | `0`      | `ResetFontSize` |
| `CTRL`      | `0`      | `ResetFontSize` |
| `SUPER`     | `t`      | `SpawnTab="CurrentPaneDomain"` |
| `CTRL+SHIFT`     | `t`      | `SpawnTab="CurrentPaneDomain"` |
| `SUPER+SHIFT` | `T`    | `SpawnTab="DefaultDomain"` |
| `SUPER`     | `w`      | `CloseCurrentTab{confirm=true}` |
| `SUPER`     | `1`      | `ActivateTab=0` |
| `SUPER`     | `2`      | `ActivateTab=1` |
| `SUPER`     | `3`      | `ActivateTab=2` |
| `SUPER`     | `4`      | `ActivateTab=3` |
| `SUPER`     | `5`      | `ActivateTab=4` |
| `SUPER`     | `6`      | `ActivateTab=5` |
| `SUPER`     | `7`      | `ActivateTab=6` |
| `SUPER`     | `8`      | `ActivateTab=7` |
| `SUPER`     | `9`      | `ActivateTab=-1` |
| `CTRL+SHIFT`     | `w`      | `CloseCurrentTab{confirm=true}` |
| `CTRL+SHIFT`     | `1`      | `ActivateTab=0` |
| `CTRL+SHIFT`     | `2`      | `ActivateTab=1` |
| `CTRL+SHIFT`     | `3`      | `ActivateTab=2` |
| `CTRL+SHIFT`     | `4`      | `ActivateTab=3` |
| `CTRL+SHIFT`     | `5`      | `ActivateTab=4` |
| `CTRL+SHIFT`     | `6`      | `ActivateTab=5` |
| `CTRL+SHIFT`     | `7`      | `ActivateTab=6` |
| `CTRL+SHIFT`     | `8`      | `ActivateTab=7` |
| `CTRL+SHIFT`     | `9`      | `ActivateTab=-1` |
| `SUPER+SHIFT` | `[` | `ActivateTabRelative=-1` |
| `SUPER+SHIFT` | `]` | `ActivateTabRelative=1` |
| `CTRL+SHIFT`     | `PageUp`      | `MoveTabRelative=-1` |
| `CTRL+SHIFT`     | `PageDown`      | `MoveTabRelative=1` |
| `SHIFT`          | `PageUp`      | `ScrollByPage=-1` |
| `SHIFT`          | `PageDown`    | `ScrollByPage=1` |
| `ALT`            | `9`    | `ShowTabNavigator` |
| `SUPER`          | `r`    | `ReloadConfiguration` |
| `CTRL+SHIFT`     | `R`    | `ReloadConfiguration` |
| `SUPER`          | `h`    | `HideApplication` (macOS only) |
| `SUPER`          | `k`    | `ClearScrollback="ScrollbackOnly"` |
| `CTRL+SHIFT`     | `K`    | `ClearScrollback="ScrollbackOnly"` |
| `SUPER`          | `f`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `F`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `X`    | `ActivateCopyMode` |
| `CTRL+SHIFT+ALT` | `"`    | `SplitVertical={domain="CurrentPaneDomain"}` |
| `CTRL+SHIFT+ALT` | `%`    | `SplitHorizontal={domain="CurrentPaneDomain"}` |
| `CTRL+SHIFT+ALT` | `LeftArrow`    | `AdjustPaneSize={"Left", 1}` |
| `CTRL+SHIFT+ALT` | `RightArrow`   | `AdjustPaneSize={"Right", 1}` |
| `CTRL+SHIFT+ALT` | `UpArrow`      | `AdjustPaneSize={"Up", 1}` |
| `CTRL+SHIFT+ALT` | `DownArrow`    | `AdjustPaneSize={"Down", 1}` |
| `CTRL+SHIFT` | `LeftArrow`    | `ActivatePaneDirection="Left"` |
| `CTRL+SHIFT` | `RightArrow`    | `ActivatePaneDirection="Right"` |
| `CTRL+SHIFT` | `UpArrow`    | `ActivatePaneDirection="Up"` |
| `CTRL+SHIFT` | `DownArrow`    | `ActivatePaneDirection="Down"` |
| `CTRL` | `Z`    | `TogglePaneZoomState` |

If you don't want the default assignments to be registered, you can
disable all of them with this configuration; if you chose to do this,
you must explicitly register every binding.

```lua
return {
  disable_default_key_bindings = true,
}
```

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
  },
}
```

The `action` and `mods` portions are described in more detail in the key assignment
information below.

The `event` portion has three components;

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


## Configuring Key Assignments


These can be overridden using the `keys` section in your `~/.wezterm.lua` config file.
For example, you can disable a default assignment like this:

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    -- Turn off the default CMD-m Hide action on macOS by making it
    -- send the empty string instead of hiding the window
    {key="m", mods="CMD", action="Nop"}
  }
}
```

The `key` value can be one of the following keycode identifiers.  Note that not
all of these are meaningful on all platforms:

`Hyper`, `Super`, `Meta`, `Cancel`, `Backspace`, `Tab`, `Clear`, `Enter`,
`Shift`, `Escape`, `LeftShift`, `RightShift`, `Control`, `LeftControl`,
`RightControl`, `Alt`, `LeftAlt`, `RightAlt`, `Menu`, `LeftMenu`, `RightMenu`,
`Pause`, `CapsLock`, `PageUp`, `PageDown`, `End`, `Home`, `LeftArrow`,
`RightArrow`, `UpArrow`, `DownArrow`, `Select`, `Print`, `Execute`,
`PrintScreen`, `Insert`, `Delete`, `Help`, `LeftWindows`, `RightWindows`,
`Applications`, `Sleep`, `Numpad0`, `Numpad1`, `Numpad2`, `Numpad3`,
`Numpad4`, `Numpad5`, `Numpad6`, `Numpad7`, `Numpad8`, `Numpad9`, `Multiply`,
`Add`, `Separator`, `Subtract`, `Decimal`, `Divide`, `NumLock`, `ScrollLock`,
`BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`,
`BrowserSearch`, `BrowserFavorites`, `BrowserHome`, `VolumeMute`,
`VolumeDown`, `VolumeUp`, `MediaNextTrack`, `MediaPrevTrack`, `MediaStop`,
`MediaPlayPause`, `ApplicationLeftArrow`, `ApplicationRightArrow`,
`ApplicationUpArrow`, `ApplicationDownArrow`.

Alternatively, a single unicode character can be specified to indicate
pressing the corresponding key.

Possible Modifier labels are:

 * `SUPER`, `CMD`, `WIN` - these are all equivalent: on macOS the `Command` key,
   on Windows the `Windows` key, on Linux this can also be the `Super` or `Hyper`
   key.  Left and right are equivalent.
 * `SHIFT` - The shift key.  Left and right are equivalent.
 * `ALT`, `OPT`, `META` - these are all equivalent: on macOS the `Option` key,
   on other systems the `Alt` or `Meta` key.  Left and right are equivalent.

You can combine modifiers using the `|` symbol (eg: `"CMD|CTRL"`).

### Leader Key

*Since: 20201031-154415-9614e117*

A *leader* key is a a modal modifier key.  If leader is specified in the
configuration then pressing that key combination will enable a virtual `LEADER`
modifier.

While `LEADER` is active, only defined key assignments that include
`LEADER` in the `mods` mask will be recognized.  Other keypresses
will be swallowed and NOT passed through to the terminal.

`LEADER` stays active until a keypress is registered (whether it
matches a key binding or not), or until it has been active for
the duration specified by `timeout_milliseconds`, at which point
it will automatically cancel itself.

Here's an example configuration using `LEADER`.  In this configuration,
pressing `CTRL-A` activates the leader key for up to 1 second (1000
milliseconds).  While `LEADER` is active, the `|` key (with no other modifiers)
will trigger the current pane to be split.

```lua
local wezterm = require 'wezterm';

return {
  -- timeout_milliseconds defaults to 1000 and can be omitted
  leader = { key="a", mods="CTRL", timeout_milliseconds=1000 },
  keys = {
    {key="|", mods="LEADER", action=wezterm.action{SplitHorizontal={domain="CurrentPaneDomain"}}},
    -- Send "CTRL-A" to the terminal when pressing CTRL-A, CTRL-A
    {key="a", mods="LEADER|CTRL", action=wezterm.action{SendString="\x01"}},
  }
}
```

# Available Actions

See the [`KeyAssignment` reference](lua/keyassignment/index.md) for information
on available actions.

