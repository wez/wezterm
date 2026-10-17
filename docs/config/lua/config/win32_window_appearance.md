---
tags:
  - appearance
---

# `win32_window_appearance = SETTING`

{{since('nightly')}}

On Windows, you can set dark or light appearance. This makes both, Windows and apps, dark or light. You can also select different appearance for Windows and apps if they prefer for example dark Windows and light apps.

By default, WezTerm follows the app appearance setting configured in Windows.

This setting allows to override it from the configuration.

The possible values for `win32_window_appearance` are:

* `"System"` - follow app appearance set in Windows settings (default)
* `"Light"` - use the light appearance
* `"Dark"` - use the dark appearance

Now, for example, all your other apps can be light while WezTerm will appear dark.

Combined with [win32_system_backdrop](win32_system_backdrop.md), WezTerm will use dark *Mica* or *Tabbed* variant.
