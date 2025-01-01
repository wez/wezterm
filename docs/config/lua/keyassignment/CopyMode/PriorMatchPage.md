# CopyMode `PriorMatchPage`

{{since('20220624-141144-bd1b7c5d')}}

Move the CopyMode/SearchMode selection to the previous matching text on the previous page of the screen, if any.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    search_mode = {
      {
        key = 'PageUp',
        mods = 'CTRL',
        action = act.CopyMode 'PriorMatchPage',
      },
    },
  },
}
```

