# `format-tab-title`

*Since: nightly builds only*

The `format-tab-title` event is emitted when the text for a tab title
needs to be recomputed.

This event is a bit special in that it is *synchronous* and must return as
quickly as possible in order to avoid blocking the GUI thread.

The most notable consequence of this is that some functions that are
asynchronous (such as
[wezterm.run_child_process](../wezterm/run_child_process.md)) are not possible
to call from inside the event handler and will generate a `format-tab-title:
runtime error: attempt to yield from outside a coroutine` error.

This example overrides the default tab title so that the background color
is blue for the active tab.  This is partially redundant because there is
already configuration for this in [tab_bar_style](../config/tab_bar_style.md),
but it demonstrates that it is possible to format more than just the text
shown in the tab.

```lua
wezterm.on("format-tab-title", function(tab, tabs, panes, config, hover)
  if tab.is_active then
    return {
      {Background={Color="blue"}},
      {Text=tab.active_pane.title},
    }
  end
  return tab.active_pane.title
end)
```

The parameters to the event are:

* `tab` - the [TabInformation](../TabInformation.md) for the active tab
* `tabs` - an array containing [TabInformation](../TabInformation.md) for each of the tabs in the window
* `panes` - an array containing [PaneInformation](../PaneInformation.md) for each of the panes in the active tab
* `config` - the effective configuration for the window
* `hover` - true if the current tab is in the hover state

The return value of the event can be:

* a string, holding the text to use for the tab title
* a table holding `FormatItem`s as used in the [wezterm.format](../wezterm/format.md) function.  This allows formatting style and color information for individual elements within the tab.

If the event encounters an error, or returns something that is not one of the
types mentioned above, then the default tab title text will be computed and
used instead.

Only the first `format-tab-title` event will be executed; it doesn't make
sense to define multiple instances of the event with multiple
`wezterm.on("format-tab-title", ...)` calls.

