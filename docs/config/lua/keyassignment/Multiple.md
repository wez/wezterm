# `Multiple`

{{since('20211204-082213-a66c61ee9')}}

Performs a sequence of multiple assignments.  This is useful when you
want a single key press to trigger multiple actions.

The example below causes `LeftArrow` to effectively type `left`:

```lua
local act = wezterm.action

config.keys = {
  {
    key = 'LeftArrow',
    action = act.Multiple {
      act.SendKey { key = 'l' },
      act.SendKey { key = 'e' },
      act.SendKey { key = 'f' },
      act.SendKey { key = 't' },
    },
  },
}
```
