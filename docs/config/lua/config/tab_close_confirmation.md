# `tab_close_confirmation`

Whether to display a confirmation prompt when the tab is closed.

Set this to `"AlwaysPrompt"` if you want to confirm closing tabs every time.
Set to `"DependsOnPanesPrompt"` if you prefer to confirm only when needed by
contained panes.

```lua
return {
  window_close_confirmation = 'DependsOnPanesPrompt',
}
```
