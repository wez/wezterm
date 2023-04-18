# `window:active_tab()`

{{since('20230408-112425-69ae8472')}}

A convenience accessor for returning the active tab within the window.

In earlier versions of wezterm, you could obtain this via:

```lua
function active_tab(window)
  for _, item in ipairs(window:tabs_with_info()) do
    if item.is_active then
      return item.tab
    end
  end
end
```
