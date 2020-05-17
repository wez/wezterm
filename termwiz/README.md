# Terminal Wizardry

This is a rust crate that provides a number of support functions for
applications interested in either displaying data to a terminal or in building
a terminal emulator.

It is currently in active development and subject to fairly wild sweeping
changes.

Included functionality:

* `Surface` models a terminal display and its component `Cell`s
* Terminal attributes are aware of modern features such as
  True Color, [Hyperlinks](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
  and will also support sixel and iterm style terminal graphics display.
* `Surface`s include a log of `Change`s and an API for consuming
  and applying deltas.  This is a powerful building block for
  synchronizing screen instances.
* Escape sequence parser decodes inscrutable escape sequences
  and gives them semantic meaning, making the code that uses
  them clearer.  The decoded escapes can be re-encoded, allowing
  applications to start with the semantic meaning and emit
  the appropriate escape sequence without embedding obscure
  binary bytes.
* `Capabilities` allows probing for terminal capabilities
  that may not be included in the system terminfo database,
  and overriding them in an embedding application.
* `Terminal` trait provides an abstraction over unix style ttys
  and Windows style console APIs.  `Change`s from `Surface`
  can be rendered to `Terminal`s.  `Terminal`s allow decoding
  mouse and keyboard inputs in both blocking or non-blocking
  mode.
* `Widget` trait allows composition of UI elements at a higher level.
* `LineEditor` implements shell-like line editing functionality.

# Windows Support

Termwiz understands how to work with both the legacy console APIs and
the new PTY and virtual terminal features available in Windows 10,
allowing for true color terminal applications on Windows 10.

# Documentation

https://docs.rs/termwiz


