# CloseCurrentTab

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


