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

### macOS and the Input Method Editor (IME)

WezTerm has support for using the operating system Input Method Editor (IME)
on macOS.  This is useful in cases where you need to type kanji.  However,
the input method editor can get in the way and has a couple of irritating
side effects such as preventing key repeat for a subset of keys.

You can control whether the IME is enabled on macOS in your configuration file:

```lua
return {
  use_ime = false,
}
```

*since: 20200620-160318-e00b076c*

The default for `use_ime` is false.  The default in earlier releases was `true`.

### Defining Assignments for key combinations that may be composed

When a key combination produces a composed key result, wezterm will look up
both the composed and uncomposed versions of the keypress in your key mappings.
If either lookup matches your assignment, that will take precedence over
the normal key processing.

## Default Shortcut / Key Binding Assignments

The default key bindings are:

| Modifiers | Key | Action |
| --------- | --- | ------ |
| `SUPER`     | `c`   | `Copy`  |
| `SUPER`     | `v`   | `Paste`  |
| `CTRL+SHIFT`     | `c`   | `Copy`  |
| `CTRL+SHIFT`     | `v`   | `Paste`  |
| `SHIFT`     | `Insert` | `Paste` |
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
| `SUPER`     | `t`      | `SpawnTab="CurrentTabDomain"` |
| `CTRL+SHIFT`     | `t`      | `SpawnTab="CurrentTabDomain"` |
| `SUPER+SHIFT` | `T`    | `SpawnTab="DefaultDomain"` |
| `SUPER`     | `w`      | `CloseCurrentTab` |
| `SUPER`     | `1`      | `ActivateTab=0` |
| `SUPER`     | `2`      | `ActivateTab=1` |
| `SUPER`     | `3`      | `ActivateTab=2` |
| `SUPER`     | `4`      | `ActivateTab=3` |
| `SUPER`     | `5`      | `ActivateTab=4` |
| `SUPER`     | `6`      | `ActivateTab=5` |
| `SUPER`     | `7`      | `ActivateTab=6` |
| `SUPER`     | `8`      | `ActivateTab=7` |
| `SUPER`     | `9`      | `ActivateTab=-1` |
| `CTRL+SHIFT`     | `w`      | `CloseCurrentTab` |
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
| `CTRL+SHIFT`     | `PAGEUP`      | `MoveTabRelative=-1` |
| `CTRL+SHIFT`     | `PAGEDOWN`      | `MoveTabRelative=1` |
| `SHIFT`          | `PAGEUP`      | `ScrollByPage=-1` |
| `SHIFT`          | `PAGEDOWN`    | `ScrollByPage=1` |
| `ALT`            | `9`    | `ShowTabNavigator` |
| `SUPER`          | `r`    | `ReloadConfiguration` |
| `CTRL+SHIFT`     | `R`    | `ReloadConfiguration` |
| `SUPER`          | `h`    | `HideApplication` (macOS only) |
| `SUPER`          | `k`    | `ClearScrollback` |
| `CTRL+SHIFT`     | `K`    | `ClearScrollback` |
| `SUPER`          | `f`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `F`    | `Search={CaseSensitiveString=""}` |
| `CTRL+SHIFT`     | `X`    | `ActivateCopyMode` |

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
| Single Left Down | `SHIFT`   | `ExtendSelectionToMouseCursor=nil`  |
| Single Left Up | `NONE`   | `CompleteSelectionOrOpenLinkAtMouseCursor`  |
| Double Left Up | `NONE`   | `CompleteSelection`  |
| Triple Left Up | `NONE`   | `CompleteSelection`  |
| Single Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Cell"`  |
| Double Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Word"`  |
| Triple Left Drag | `NONE`   | `ExtendSelectionToMouseCursor="Line"`  |
| Single Middle Down | `NONE`   | `Paste`  |

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
      action="CompleteSelection",
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

# Possible Actions

Possible actions are listed below.

## SpawnTab

Create a new tab in the current window.  The argument defines to which *domain* the tab belongs:

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- Create a new tab in the default domain
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab="DefaultDomain"}},
    -- Create a new tab in the same domain as the current tab
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab="CurrentTabDomain"}},
    -- Create a tab in a named domain
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab={DomainName="unix"}}},
  }
}
```

## SpawnWindow

Create a new window containing a tab from the default tab domain.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="n", mods="SHIFT|CTRL", action="SpawnWindow"},
  }
}
```

## SpawnCommandInNewWindow / SpawnCommandInNewTab

Spawn a new tab either into the current window or into a brand new window.
The argument controls which command is run in the tab; it is a lua table
with the following fields:

* `args` - the argument array specifying the command and its arguments.
  If omitted, the default program will be run.
* `cwd` - the current working directory to set for the command.
* `set_environment_variables` - a table specifying key/value pairs to
  set in the environment
* `domain` - specifies the domain into which the tab will be spawned.
  See `SpawnTab` for examples.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    -- CMD-y starts `top` in a new window
    {key="y", mods="CMD", action=wezterm.action{SpawnCommandInNewWindow={
      args={"top"}
    }}},
  }
}
```

## ToggleFullScreen

Toggles full screen mode for the current window.  (But see:
<https://github.com/wez/wezterm/issues/177>)

```lua
return {
  keys = {
    {key="n", mods="SHIFT|CTRL", action="ToggleFullScreen"},
  }
}
```

## Copy

Copy the selection to the clipboard.  On X11 systems, this populates both the
Clipboard and the Primary Selection.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="c", mods="SHIFT|CTRL", action="Copy"},
  }
}
```

## Paste

Paste the clipboard to the current tab.  On X11 systems, this copies from the
Clipboard rather than the Primary Selection.  See also `PastePrimarySelection`.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="Paste"},
  }
}
```

## PastePrimarySelection

X11: Paste the Primary Selection to the current tab.
On other systems, this behaves identically to `Paste`.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="v", mods="SHIFT|CTRL", action="PastePrimarySelection"},
  }
}
```

## ActivateTabRelative

Activate a tab relative to the current tab.  The argument value specifies an
offset. eg: `-1` activates the tab to the left of the current tab, while `1`
activates the tab to the right.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="{", mods="SHIFT|ALT", action=wezterm.action{ActivateTabRelative=-1}},
    {key="}", mods="SHIFT|ALT", action=wezterm.action{ActivateTabRelative=1}},
  }
}
```

## ActivateTab

Activate the tab specified by the argument value. eg: `0` activates the
leftmost tab, while `1` activates the second tab from the left, and so on.

*since: 20200620-160318-e00b076c*

`ActivateTab` now accepts negative numbers; these wrap around from the start
of the tabs to the end, so `-1` references the right-most tab, `-2` the tab
to its left and so on.


```lua
local wezterm = require 'wezterm';

local mykeys = {}
for i = 1, 8 do
  -- CTRL+ALT + number to activate that tab
  table.insert(mykeys, {
    key=tostring(i),
    mods="CTRL|ALT",
    action=wezterm.action{ActivateTab=i-1},
  })
  -- F1 through F8 to activate that tab
  table.insert(mykeys, {
    key="F" .. tostring(i),
    action=wezterm.action{ActivateTab=i-1},
  })
end

return {
  keys = mykeys,
}
```

## IncreaseFontSize

Increases the font size of the current window by 10%

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="=", mods="CTRL", action="IncreaseFontSize"},
  }
}
```
  
## DecreaseFontSize

Decreases the font size of the current window by 10%

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="-", mods="CTRL", action="DecreaseFontSize"},
  }
}
```

## ResetFontSize

Reset the font size for the current window to the value in your configuration

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="0", mods="CTRL", action="ResetFontSize"},
  }
}
```

## SendString

Sends the string specified argument to the terminal in the current tab, as
though that text were literally typed into the terminal.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    {key="m", mods="CMD", action=wezterm.action{SendString="Hello"}},
  }
}
```

## DisableDefaultAssignment

Has no special meaning of its own; this action will undo the registration
of a default assignment if that key/mouse/modifier combination is one of the
default assignments and cause the key press to be propagated through
to the tab for processing.

```lua
return {
  keys = {
    -- Turn off the default CMD-m Hide action, allowing CMD-m to
    -- be potentially recognized and handled by the tab
    {key="m", mods="CMD", action="DisableDefaultAssignment"},
  }
}
```

## Nop

Causes the key press to have no effect; it behaves as though those
keys were not pressed.

```lua
return {
  keys = {
    -- Turn off any side effects from pressing CMD-m
    {key="m", mods="CMD", action="Nop"},
  }
}
```

## Hide

Hides the current window

```lua
return {
  keys = {
    {key="h", mods="CMD", action="Hide"},
  }
}
```

## HideApplication

On macOS, hide the WezTerm application.

```lua
return {
  keys = {
    {key="h", mods="CMD", action="HideApplication"},
  }
}
```

## QuitApplication

Terminate the WezTerm application, killing all tabs.

```lua
return {
  keys = {
    {key="q", mods="CMD", action="QuitApplication"},
  }
}
```

## Show

Shows the current window.

## CloseCurrentTab

Equivalent to clicking the `x` on the window title bar to close it: Closes the
current tab.  If that was the last tab, closes that window.  If that was the
last window, wezterm terminates.

```lua
return {
  keys = {
    {key="w", mods="CMD", action="CloseCurrentTab"},
  }
}
```

## MoveTabRelative

Move the current tab relative to its peers.  The argument specifies an
offset. eg: `-1` moves the tab to the left of the current tab, while `1` moves
the tab to the right.

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    {key="{", mods="SHIFT|ALT", action=wezterm.action{MoveTabRelative=-1}},
    {key="}", mods="SHIFT|ALT", action=wezterm.action{MoveTabRelative=1}},
  }
}
```

## MoveTab

Move the tab so that it has the index specified by the argument. eg: `0`
moves the tab to be  leftmost, while `1` moves the tab so that it is second tab
from the left, and so on.

```lua
local wezterm = require 'wezterm';

local mykeys = {}
for i = 1, 8 do
  -- CTRL+ALT + number to move to that position
  table.insert(mykeys, {
    key=tostring(i),
    mods="CTRL|ALT",
    action=wezterm.action{Move=i-1},
  })
end

return {
  keys = mykeys,
}
```

## ScrollByPage

Adjusts the scroll position by the number of pages specified by the argument.
Negative values scroll upwards, while positive values scroll downwards.

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    {key="PageUp", mods="SHIFT", action=wezterm.action{ScrollByPage=-1}},
    {key="PageDown", mods="SHIFT", action=wezterm.action{ScrollByPage=1}},
  }
}
```

## ClearScrollback

Clears the lines that have scrolled off the top of the viewport, resetting
the scrollbar thumb to the full height of the window.

```lua
return {
  keys = {
    {key="K", mods="CTRL|SHIFT", action="ClearScrollback"}
  }
}
```

## ReloadConfiguration

Explicitly reload the configuration.

```lua
return {
  keys = {
    {key="r", mods="CMD|SHIFT", action="ReloadConfiguration"},
  }
}
```

## ShowLauncher

Activate the [Launcher Menu](launch.html#the-launcher-menu)
in the current tab.

```lua
return {
  keys = {
    {key="l", mods="ALT", action="ShowLauncher"},
  }
}
```

## ShowTabNavigator

Activate the tab navigator UI in the current tab.  The tab
navigator displays a list of tabs and allows you to select
and activate a tab from that list.

```lua
return {
  keys = {
    {key="F9", mods="ALT", action="ShowTabNavigator"},
  }
}
```

## SelectTextAtMouseCursor

Initiates selection of text at the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

## ExtendSelectionToMouseCursor

Extends the current text selection to the current mouse cursor position.
The mode argument can be one of `Cell`, `Word` or `Line` to control
the scope of the selection.

## OpenLinkAtMouseCursor

If the current mouse cursor position is over a cell that contains
a hyperlink, this action causes that link to be opened.

## CompleteSelection

Completes an active text selection process; the selection range is
marked closed and then the selected text is copied as though the
`Copy` action was executed.

## CompleteSelectionOrOpenLinkAtMouseCursor

If a selection is in progress, acts as though `CompleteSelection` was
triggered.  Otherwise acts as though `OpenLinkAtMouseCursor` was
triggered.

## Search

*since: 20200607-144723-74889cd4*

This action will trigger the search overlay for the current tab.
It accepts a typed pattern string as its parameter, allowing for
`Regex`, `CaseSensitiveString` and `CaseInSensitiveString` as
pattern matching types.

The supported [regular expression syntax is described
here](https://docs.rs/regex/1.3.9/regex/#syntax).


```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- search for things that look like git hashes
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={Regex="[a-f0-9]{6,}"}}},
    -- search for the lowercase string "hash" matching the case exactly
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={CaseSensitiveString="hash"}}},
    -- search for the string "hash" matching regardless of case
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={CaseInSensitiveString="hash"}}},
  },
}
```

## ActivateCopyMode

*since: 20200607-144723-74889cd4*

Activates copy mode!

[Learn more about copy mode](../copymode.html)
