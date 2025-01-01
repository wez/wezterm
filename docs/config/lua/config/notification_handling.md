---
tags:
  - notifications
---

# `notification_handling = "AlwaysShow"`

{{since('20240127-113634-bbcac864')}}

This option controls how wezterm behaves when a toast notification escape
sequence is received.

The following escape sequences will generate a toast notification:

```console
$ printf "\e]777;notify;%s;%s\e\\" "title" "body"
```

```console
$ printf "\e]9;%s\e\\" "hello there"
```

This configuration option can have one of the following values,
which have the following effects:

 * `AlwaysShow` - Show the notification regardless of the current focus
 * `NeverShow` - Never show the notification
 * `SuppressFromFocusedPane` - Show the notification unless it was generated from the currently focused pane
 * `SuppressFromFocusedTab` - Show the notification unless it was generated from the currently focused tab
 * `SuppressFromFocusedWindow` - Show the notification unless it was generated from the currently focused window
