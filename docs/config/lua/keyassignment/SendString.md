# `SendString`

Sends the string specified argument to the terminal in the current tab, as
though that text were literally typed into the terminal.

```lua
config.keys = {
  { key = 'm', mods = 'CMD', action = wezterm.action.SendString 'Hello' },
}
```

You can also emit escape sequences using `SendString`.  This example shows
how to bind Alt-LeftArrow/RightArrow to the Alt-b/f, an emacs style
keybinding for moving backwards/forwards through a word in a line editor.

`\x1b` is the ESC character:

```lua
local act = wezterm.action

config.keys = {
  -- Make Option-Left equivalent to Alt-b which many line editors interpret as backward-word
  { key = 'LeftArrow', mods = 'OPT', action = act.SendString '\x1bb' },
  -- Make Option-Right equivalent to Alt-f; forward-word
  { key = 'RightArrow', mods = 'OPT', action = act.SendString '\x1bf' },
}
```

See also [SendKey](SendKey.md) which makes the example above much more convenient,
and [Multiple](Multiple.md) for combining multiple actions in a single press.
