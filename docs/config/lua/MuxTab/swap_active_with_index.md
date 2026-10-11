# `tab:swap_active_with_index(pane_index, keep_focus)`

Swaps the position of the active pane with the specified pane index. If `keep_focus` is false, focus will change to the selected pane, otherwise focus stays on the currently active pane in its new position.

# Example: tmux-like pane marking and swapping

```lua
local marked_pane = nil

wezterm.on('mark', function(window, pane)
  id = pane:pane_id()
  tab = window:active_tab()

  marked_pane = { id, tab }
end)

wezterm.on('swap-pane', function(window, pane)
  if not marked_pane then
    wezterm.log_error 'No marked pane'
    return
  end

  local cur_tab = window:active_tab()

  -- Do nothing if marked pane is same as current
  if marked_pane[1] == cur_tab:active_pane():pane_id() then
    return
  end

  if marked_pane[2]:tab_id() ~= cur_tab:tab_id() then
    wezterm.log_error 'Pane is on another tab'
    return
  end

  cur_tab:swap_active_with_id(marked_pane[1], true)
end)

config = {
  keys = {
    { key = 'm', mods = 'CTRL', action = wezterm.action.EmitEvent 'mark' },
    { key = 'm', mods = 'ALT', action = wezterm.action.EmitEvent 'swap-pane' },
  },
}
```
