---
title: wezterm.background_child_process
tags:
 - utility
 - open
 - spawn
---

# `wezterm.background_child_process(args)`

{{since('20211204-082213-a66c61ee9')}}

This function accepts an argument list; it will attempt to spawn that command
in the background.

May generate an error if the command is not able to be spawned (eg: perhaps
the executable doesn't exist), but not all operating systems/environments
report all types of spawn failures immediately upon spawn.

This function doesn't return any value.

This example shows how you might set up a custom key assignment that opens
the terminal background image in a separate image viewer process:

```lua
local wezterm = require 'wezterm'

return {
  window_background_image = '/home/wez/Downloads/sunset-american-fork-canyon.jpg',
  keys = {
    {
      mods = 'CTRL|SHIFT',
      key = 'm',
      action = wezterm.action_callback(function(win, pane)
        wezterm.background_child_process {
          'xdg-open',
          win:effective_config().window_background_image,
        }
      end),
    },
  },
}
```

See also [run_child_process](run_child_process.md)

