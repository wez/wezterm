---
tags:
  - spawn
---
# `default_prog`

If no `prog` is specified on the command line, use this
instead of running the user's shell.

For example, to have `wezterm` always run `top` by default,
you'd use this:

```lua
config.default_prog = { 'top' }
```

`default_prog` is implemented as an array where the 0th element
is the command to run and the rest of the elements are passed
as the positional arguments to that command.

See also: [Launching Programs](../../launch.md)
