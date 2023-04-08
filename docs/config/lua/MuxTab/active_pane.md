# `tab:active_pane()`

{{since('20230408-112425-69ae8472')}}

A convenience accessor for returning the active pane in the tab.

In earlier versions of wezterm, you could obtain this via:

```lua
function active_pane(tab)
  for _, item in ipairs(tab:panes_with_info()) do
    if item.is_active then
      return item.pane
    end
  end
end
```

