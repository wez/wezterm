---
tags:
  - keys
---
# `xim_im_name`

{{since('20220101-133340-7edc5b5a')}}

Explicitly set the name of the IME server to which wezterm will connect
via the XIM protocol when using X11 and [use_ime](use_ime.md) is `true`.

By default, this option is not set which means that wezterm will consider
the value of the `XMODIFIERS` environment variable.

If for some reason the environment isn't set up correctly, or you want
to quickly evaluate a different input method server, then you could
update your config to specify it explicitly:

```lua
config.xim_im_name = 'fcitx'
```

will cause wezterm to connect to fcitx regardless of the value of `XMODIFIERS`.

