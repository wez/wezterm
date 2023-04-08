---
title: wezterm.shell_join_args
tags:
 - utility
 - open
 - spawn
 - string
---
# wezterm.shell_join_args({"foo", "bar"})

{{since('20220807-113146-c2fee766')}}

`wezterm.shell_join_args` joins together its array arguments by applying posix
style shell quoting on each argument and then adding a space.

```
> wezterm.shell_join_args{"foo", "bar"}
"foo bar"
> wezterm.shell_join_args{"hello there", "you"}
"\"hello there\" you"
```

This is useful to safely construct command lines that you wish to pass to the shell.
