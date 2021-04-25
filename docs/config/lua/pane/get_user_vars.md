# `pane:get_user_vars()`

*Since: nightly builds only*

Returns a table holding the user variables that have been assigned
to this pane.

User variables are set using an escape sequence defined by iterm2, but
also recognized by wezterm; this example sets the `foo` user variable
to the value `bar`:

```bash
printf "\033]1337;SetUserVar=%s=%s\007" foo `echo -n bar | base64`
```

you're then able to access this in your wezterm config:

```lua
wezterm.log_info("foo var is " .. pane:get_user_vars().foo)
```

