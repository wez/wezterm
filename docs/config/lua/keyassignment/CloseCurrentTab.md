# CloseCurrentTab

Closes the current tab, terminating all contained panes.  If that was the last
tab, closes that window.  If that was the last window, wezterm terminates.

```lua
return {
  keys = {
    {key="w", mods="CMD", action="CloseCurrentTab"},
  }
}
```

*Since: 20201031-154415-9614e117*

`CloseCurrentTab` was changed so that it requires
a boolean `confirm` parameter:

```lua
return {
  keys = {
    {key="w", mods="CMD",
     action=wezterm.action{CloseCurrentTab={confirm=true}}
  }
}
```

When `confirm` is true, an overlay will render over the tab to ask you to
confirm whether you want to close it.  See also
[skip_close_confirmation_for_processes_named](../config/skip_close_confirmation_for_processes_named.md).


If `confirm` is false then this action will immediately close
the tab and terminates its panes without prompting.

