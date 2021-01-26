# ClearScrollback

Clears the lines that have scrolled off the top of the viewport, resetting
the scrollbar thumb to the full height of the window, and additionally the
viewport depending on the argument:

```lua
return {
  keys = {
    -- Clears only the scrollback and leaves the viewport intact.
    -- This is the default behavior.
    {key="K", mods="CTRL|SHIFT", action=wezterm.action{ClearScrollback="ScrollbackOnly"}}
    -- Clears the scrollback and viewport leaving the prompt line the new first line.
    {key="K", mods="CTRL|SHIFT", action=wezterm.action{ClearScrollback="ScrollbackAndViewport"}}
  }
}
```
