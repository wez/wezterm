# `window:toast_notification(title, message,  [url])`

*Since: nightly builds only*

Generates a desktop "toast notification" with the specified *title* and *message*.

An optional *url* parameter can be provided; clicking on the notification will
open that URL.

The notification will persist on screen until dismissed or clicked.

This example will display a notification whenever a window has its configuration
reloaded.  It's not an ideal implementation because there may be multiple windows
and thus multiple notifications:

```lua
local wezterm = require 'wezterm'

wezterm.on("window-config-reloaded", function(window, pane)
  window:toast_notification("wezterm", "configuration reloaded!")
end)

return {}
```
