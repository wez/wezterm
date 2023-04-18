---
tags:
  - appearance
---
# `integrated_title_button_style = STYLE`

{{since('20230408-112425-69ae8472')}}

Configures the visual style of the tabbar-integrated titlebar button
replacements that are shown when `window_decorations =
"INTEGRATED_BUTTONS|RESIZE"`.

Possible styles are:

* `"Windows"` - draw Windows-style buttons
* `"Gnome"` - draw Adwaita-style buttons
* `"MacOsNative"` - on macOS only, move the native macOS buttons into the tab bar.

The default value is `"MacOsNative"` on macOS systems, but `"Windows"` on other
systems.
