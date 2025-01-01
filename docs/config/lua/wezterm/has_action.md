---
title: wezterm.has_action
tags:
 - utility
 - version
---

# wezterm.has_action(NAME)

{{since('20230408-112425-69ae8472')}}

Returns true if the string *NAME* is a valid key assignment action variant
that can be used with [wezterm.action](action.md).

This is useful when you want to use a wezterm configuration across multiple
different versions of wezterm.

```lua
if wezterm.has_action 'PromptInputLine' then
  table.insert(config.keys, {
    key = 'p',
    mods = 'LEADER',
    action = wezterm.action.PromptInputLine {
      -- other parameters here
    },
  })
end
```
