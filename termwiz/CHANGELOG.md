## termwiz-0.20.0:

* Support for horizontal scroll wheel event decoding. Thanks to
  [@tlinford](https://github.com/tlinford)!
  [#2813](https://github.com/wezterm/wezterm/issues/2813)
* Correctly recognize `Alt-[` keyboard events. Thanks to
  [@imsnif](https://github.com/imsnif)!
  [#3009](https://github.com/wezterm/wezterm/pull/3009)
* Adjusted Line clustering when bidi is disabled to improve perf when
  used in wezterm
* Fix crash bug when using to TeenyString inside Cell with Rust 1.67
* Updated nerdfonts metadata for v2.3.3

## termwiz-0.19.0:

* Added `Action::PrintString` to more efficiently accumulate sequences of
  printed characters.
* Fixed build on 32-bit platforms
* Fixed build on Android systems
* Updates for Unicode 15
* Widgets can now control cursor visibility
* BREAKING: We now request modifyOtherKeys when setting up the unix terminal.
  As a consequence, CTRL keys like `CTRL-C` are now reported as
  `CTRL-lower-case-c` rather than `CTRL-upper-case-C`. We do this even when
  modifyOtherKeys isn't active for the sake of overall consistency.
