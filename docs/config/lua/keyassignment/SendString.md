# SendString

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

You can also emit escape sequences using `SendString`.  This example shows
how to bind Alt-LeftArrow/RightArrow to the Alt-b/f, an emacs style
keybinding for moving backwards/forwards through a word in a line editor.

`\x1b` is the ESC character:

```lua
local wezterm = require 'wezterm';

return {
  keys = {
    -- Make Option-Left equivalent to Alt-b which many line editors interpret as backward-word
    {key="LeftArrow", mods="OPT", action=wezterm.action{SendString="\x1bb"}},
    -- Make Option-Right equivalent to Alt-f; forward-word
    {key="RightArrow", mods="OPT", action=wezterm.action{SendString="\x1bf"}},
  }
}
```

