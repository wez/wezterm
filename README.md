# Terminal Wizardry

[![Build Status](https://travis-ci.org/wez/termwiz.svg?branch=master)](https://travis-ci.org/wez/termwiz)

This is a rust crate that provides a number of support functions
for applications interesting in either displaying data to a terminal
or in building a terminal emulator.

It is currently in active development and subject to fairly wild
sweeping changes.

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

# Documentation

Until this goes up on crates.io, run:

```
$ cargo doc --open
```

to build and browse the docs locally.

## TODO

 * [ ] Load key mapping information from terminfo
 * [ ] Look at unicode width and graphemes for cells
 * [ ] ensure that sgr is reset to default on drop
 * [ ] Option to use alt screen when going raw
 * [x] Mouse reporting mode (and restore on drop)
 * [x] Bracketed paste mode (and restore on drop)

## Windows

Testing via Wine:

```
sudo apt install gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo build --target=x86_64-pc-windows-gnu  --example hello
```

Then, from an X session of some kind:

```
wineconsole cmd.exe
```

and from there you can launch the generated .exe files; they are found under `target/x86_64-pc-windows-gnu/debug`


