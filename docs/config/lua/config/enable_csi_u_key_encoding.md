# `enable_csi_u_key_encoding = false`

This option affects how key presses are fed to the terminal; after processing
any key binding assignments, if the key didn't match an assignment it is passed
down to the terminal which then encodes the key press as a byte sequence to
send to the application running in the terminal.

By default, wezterm aims to be compatible with the encoding used by `xterm`.

In that encoding scheme there are some key combinations that have an ambiguous
representation.

Setting `enable_csi_u_key_encoding = true` will switch to an alternative
encoding scheme that is [described here](http://www.leonerd.org.uk/hacks/fixterms/)
that removes the ambiguity in a mostly-backwards-compatible way, but that
requires that applications also know about this encoding scheme to have
the best results.

The default for this option is `false`.

