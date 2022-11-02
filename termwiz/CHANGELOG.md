
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
