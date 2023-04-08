---
tags:
  - keys
---
# `enable_csi_u_key_encoding = false`

When set to `true`, the [keyboard encoding](../../key-encoding.md) will be
changed to use the scheme that is [described
here](http://www.leonerd.org.uk/hacks/fixterms/).

It is not recommended to enable this option as it does change the behavior of
some keys in backwards incompatible ways and there isn't a way for applications
to detect or request this behavior.

The default for this option is `false`.

Note that [allow_win32_input_mode](allow_win32_input_mode.md) takes
precedence over this option.
