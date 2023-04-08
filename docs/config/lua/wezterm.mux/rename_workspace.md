# `wezterm.mux.rename_workspace(old, new)`

{{since('20230408-112425-69ae8472')}}

Renames the workspace *old* to *new*.

```lua
wezterm.mux.rename_workspace(
  wezterm.mux.get_active_workspace(),
  'something different'
)
```
