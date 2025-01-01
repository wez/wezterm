# `format-window-title`

{{since('20210502-154244-3f7122cb')}}

The `format-window-title` event is emitted when the text for the window title
needs to be recomputed.

This event is a bit special in that it is *synchronous* and must return as
quickly as possible in order to avoid blocking the GUI thread.

The most notable consequence of this is that some functions that are
asynchronous (such as
[wezterm.run_child_process](../wezterm/run_child_process.md)) are not possible
to call from inside the event handler and will generate a `format-window-title:
runtime error: attempt to yield from outside a coroutine` error.

This example overrides the default window title with code that is equivalent
to the default processing--not very useful except as a starting point for
making your own title text:

```lua
wezterm.on('format-window-title', function(tab, pane, tabs, panes, config)
  local zoomed = ''
  if tab.active_pane.is_zoomed then
    zoomed = '[Z] '
  end

  local index = ''
  if #tabs > 1 then
    index = string.format('[%d/%d] ', tab.tab_index + 1, #tabs)
  end

  return zoomed .. index .. tab.active_pane.title
end)
```

The parameters to the event are:

* `tab` - the [TabInformation](../TabInformation.md) for the active tab
* `pane` - the [PaneInformation](../PaneInformation.md) for the active pane
* `tabs` - an array containing [TabInformation](../TabInformation.md) for each of the tabs in the window
* `panes` - an array containing [PaneInformation](../PaneInformation.md) for each of the panes in the active tab
* `config` - the effective configuration for the window

The return value of the event should be a string, and if it is then it will be
used as the title text in the window title bar.

If the event encounters an error, or returns something that is not a string,
then the default window title text will be computed and used instead.

Only the first `format-window-title` event will be executed; it doesn't make
sense to define multiple instances of the event with multiple
`wezterm.on("format-window-title", ...)` calls.

