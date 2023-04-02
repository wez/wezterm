# `wezterm.mux.rename_workspace(old, new)`

{{since('nightly')}}

Renames the workspace *old* to *new*.

```lua
wezterm.mux.rename_workspace(
  wezterm.mux.get_active_workspace(),
  'something different'
)
```
