# `pane:get_user_vars()`

{{since('20210502-130208-bff6815d')}}

Returns a table holding the user variables that have been assigned
to this pane.

User variables are set using an escape sequence defined by iterm2, but
also recognized by wezterm; this example sets the `foo` user variable
to the value `bar`:

```bash
# This function emits an OSC 1337 sequence to set a user var
# associated with the current terminal pane.
# It requires the `base64` utility to be available in the path.
# This function is included in the wezterm shell integration script, but
# is reproduced here for clarity
__wezterm_set_user_var() {
  if hash base64 2>/dev/null ; then
    if [[ -z "${TMUX}" ]] ; then
      printf "\033]1337;SetUserVar=%s=%s\007" "$1" `echo -n "$2" | base64`
    else
      # <https://github.com/tmux/tmux/wiki/FAQ#what-is-the-passthrough-escape-sequence-and-how-do-i-use-it>
      # Note that you ALSO need to add "set -g allow-passthrough on" to your tmux.conf
      printf "\033Ptmux;\033\033]1337;SetUserVar=%s=%s\007\033\\" "$1" `echo -n "$2" | base64`
    fi
  fi
}

__wezterm_set_user_var "foo" "bar"
```

you're then able to access this in your wezterm config:

```lua
wezterm.log_info('foo var is ' .. pane:get_user_vars().foo)
```

Setting a user var will generate events in the window that contains
the corresponding pane:

* [user-var-changed](../window-events/user-var-changed.md), which
  allows you to directly take action when a var is set/changed.
* [update-status](../window-events/update-status.md) which allows you to update left/right status items
* the title and tab bar area will then update and trigger any associated events as part of that update

The user var change event will propagate to all connected multiplexer clients.

