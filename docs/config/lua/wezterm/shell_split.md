# wezterm.shell_split(line)

*Since: nightly builds only*

Splits a command line into an argument array according to posix shell rules.

```
> wezterm.shell_split("ls -a")
[
    "ls",
    "-a",
]
```

```
> wezterm.shell_split("echo 'hello there'")
[
    "echo",
    "hello there",
]
```
