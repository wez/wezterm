
main:

* BREAKING: We now request modifyOtherKeys when setting up the unix terminal.
  As a consequence, CTRL keys like `CTRL-C` are now reported as
  `CTRL-lower-case-c` rather than `CTRL-upper-case-C`. We do this even when
  modifyOtherKeys isn't active for the sake of overall consistency.
