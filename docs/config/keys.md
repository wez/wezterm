## Configuring Key Assignments

The default key table assignments can be overridden or extended using the
`keys` section in your `~/.wezterm.lua` config file.  For example, you can
disable a default assignment like this:

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

The `action` value can be one of the [available key
assignments](lua/keyassignment/index.md).  Every action has an example that shows
how to use it.

Possible Modifier labels are:

 * `SUPER`, `CMD`, `WIN` - these are all equivalent: on macOS the `Command` key,
   on Windows the `Windows` key, on Linux this can also be the `Super` or `Hyper`
   key.  Left and right are equivalent.
 * `CTRL` - The control key.  Left and right are equivalent.
 * `SHIFT` - The shift key.  Left and right are equivalent.
 * `ALT`, `OPT`, `META` - these are all equivalent: on macOS the `Option` key,
   on other systems the `Alt` or `Meta` key.  Left and right are equivalent.
 * `LEADER` - a special modal modifier state managed by `wezterm`. See [Leader Key](#leader-key) for more information.
 * `VoidSymbol` - This keycode is emitted in special cases where the original
   function of the key has been removed. Such as in Linux and using `setxkbmap`.
   `setxkbmap -option caps:none`. The `CapsLock` will no longer function as
   before in all applications, instead emitting `VoidSymbol`.

You can combine modifiers using the `|` symbol (eg: `"CMD|CTRL"`).

The `key` value can be one of the following keycode identifiers.  Note that not
all of these are meaningful on all platforms:

`Hyper`, `Super`, `Meta`, `Cancel`, `Backspace`, `Tab`, `Clear`, `Enter`,
`Shift`, `Escape`, `LeftShift`, `RightShift`, `Control`, `LeftControl`,
`RightControl`, `Alt`, `LeftAlt`, `RightAlt`, `Menu`, `LeftMenu`, `RightMenu`,
`Pause`, `CapsLock`, `VoidSymbol`, `PageUp`, `PageDown`, `End`, `Home`, `LeftArrow`,
`RightArrow`, `UpArrow`, `DownArrow`, `Select`, `Print`, `Execute`,
`PrintScreen`, `Insert`, `Delete`, `Help`, `LeftWindows`, `RightWindows`,
`Applications`, `Sleep`, `Numpad0`, `Numpad1`, `Numpad2`, `Numpad3`,
`Numpad4`, `Numpad5`, `Numpad6`, `Numpad7`, `Numpad8`, `Numpad9`, `Multiply`,
`Add`, `Separator`, `Subtract`, `Decimal`, `Divide`, `NumLock`, `ScrollLock`,
`BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`,
`BrowserSearch`, `BrowserFavorites`, `BrowserHome`, `VolumeMute`,
`VolumeDown`, `VolumeUp`, `MediaNextTrack`, `MediaPrevTrack`, `MediaStop`,
`MediaPlayPause`, `ApplicationLeftArrow`, `ApplicationRightArrow`,
`ApplicationUpArrow`, `ApplicationDownArrow`, `F1`, `F2`, `F3`, `F4`,
`F5`, `F6`, `F7`, `F8`, `F9`, `F10`, `F11`, `F12`, `F13`, `F14`, `F15`,
`F16`, `F17`, `F18`, `F19`, `F20`, `F21`, `F22`, `F23`, `F24`.

Alternatively, a single unicode character can be specified to indicate
pressing the corresponding key.

Pay attention to the case of the text that you use and the state of the `SHIFT` modifier, as `key="A"` will match 

### Physical vs Mapped Key Assignments

{{since('20220319-142410-0fcdea07')}}

The `key` value can refer either to the physical position of a key on an ANSI
US keyboard or to the post-keyboard-layout-mapped value produced by a key
press.

You can explicitly assign using the physical position by adding a `phys:` prefix
to the value, for example: `key="phys:A"`.  This will match key presses for
the key that would be in the position of the `A` key on an ANSI US keyboard.

You can explicitly assign the mapped key by adding a `mapped:` prefix to the
value, for example: `key="mapped:a"` will match a key press where the OS
keyboard layout produces `a`, regardless of its physical position.

If you omit an explicit prefix, wezterm will assume `phys:` and use the
physical position of the specified key.

The default key assignments listed above use `phys:`.  In previous releases
there was no physical position support and those assignments were all `mapped:`.

When upgrading from earlier releases, if you had `{key="N", mods="CMD", ..}` in
your config, you will need to change it to either
`{key="N", mods="CMD|SHIFT", ..}` or `{key="mapped:N", mods="CMD", ..}`
in order to continue to respect the `SHIFT` modifier.

{{since('20220408-101518-b908e2dd')}}

A new `key_map_preference` option controls how keys without an explicit `phys:`
or `mapped:` prefix are treated. If `key_map_preference = "Mapped"` (the
default), then `mapped:` is assumed. If `key_map_preference = "Physical"` then
`phys:` is assumed.

The default key assignments will respect `key_map_preference`.

### Raw Key Assignments

In some cases, `wezterm` may not know how to represent a key event in either
its `phys:` or `mapped:` forms.  In that case, you may wish to define an
assignment in terms of the underlying operating system key code, using a `raw:`
prefix.

Similar in concept to the `phys:` mapping described above, the `raw:` mapping
is independent of the OS keyboard layout.  Raw codes are hardware and windowing system dependent, so there is no portable way to list which key does what.

To discover these values, you can set [debug_key_events =
true](lua/config/debug_key_events.md) and press the keys of
interest.

You can specify a raw key value of 123 by using `key="raw:123"` in your config
rather than one of the other key values.

### Leader Key

{{since('20201031-154415-9614e117')}}

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
-- timeout_milliseconds defaults to 1000 and can be omitted
config.leader = { key = 'a', mods = 'CTRL', timeout_milliseconds = 1000 }
config.keys = {
  {
    key = '|',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.SplitHorizontal { domain = 'CurrentPaneDomain' },
  },
  -- Send "CTRL-A" to the terminal when pressing CTRL-A, CTRL-A
  {
    key = 'a',
    mods = 'LEADER|CTRL',
    action = wezterm.action.SendKey { key = 'a', mods = 'CTRL' },
  },
}
```

### VoidSymbol

{{since('20210814-124438-54e29167')}}

On X11 systems, If you decide to change certain keys on the keyboard to
`VoidSymbol` (like `CapsLock`), then you can utilize it as a `LEADER` or any
other part of key bindings. The following example now uses `VoidSymbol` and
uses `CapsLock` as a `LEADER` without it affecting the shift / capital state as
long as you have `setxkbmap -option caps:none` configured.

```lua
-- timeout_milliseconds defaults to 1000 and can be omitted
-- for this example use `setxkbmap -option caps:none` in your terminal.
config.leader = { key = 'VoidSymbol', mods = '', timeout_milliseconds = 1000 }
config.keys = {
  {
    key = '|',
    mods = 'LEADER|SHIFT',
    action = wezterm.action.SplitHorizontal { domain = 'CurrentPaneDomain' },
  },
  {
    key = '-',
    mods = 'LEADER',
    action = wezterm.action.SplitVertical { domain = 'CurrentPaneDomain' },
  },
}
```

# Available Actions

See the [`KeyAssignment` reference](lua/keyassignment/index.md) for information
on available actions.

