# Passing Data from a pane to Lua

After spawning a program into a pane, a terminal emulator has no guaranteed
reliable way to reason about what might be happening inside the pane; the only
official system-provided means of interaction is through a limited PTY
interface that basically provides only input and output streams and a way to
communicate the screen size to the pane.

While wezterm provides a few functions that can help to peek into locally
running processes, those cannot be used with remote processes when you're
ssh'ing to a remote host, for example.

Here are a few options you might consider using, depending on your needs.

We'll start with a very general but powerful mechanism:

## User Vars

You can use an escape sequence to set a key/value pair in a terminal pane.
These *user vars* are similar in some ways to environment variables but are
scoped to the terminal pane and cannot be read by applications running in the
pane, only written.

Here's an example of setting the `foo` user variable to the value `bar`:

```bash
printf "\033]1337;SetUserVar=%s=%s\007" foo `echo -n bar | base64`
```

Note that the value must be base64 encoded.

Setting a user var will generate events in the window that contains
the corresponding pane:

* [user-var-changed](../config/lua/window-events/user-var-changed.md), which
  allows you to directly take action when a var is set/changed.
* [update-status](../config/lua/window-events/update-status.md) which allows you to update left/right status items
* the title and tab bar area will then update and trigger any associated events as part of that update

The user var change event will propagate to all connected multiplexer clients.

You can access the complete set of user vars in a given pane by calling
[pane:get_user_vars()](../config/lua/pane/get_user_vars.md), or by accessing
the `user_vars` field in a [PaneInformation](../config/lua/PaneInformation.md)
struct.

In this example, an alias is used to set a user var named PROG to something
when running various commands:

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

function _run_prog() {
    # set PROG to the program being run
    __wezterm_set_user_var "PROG" "$1"

    # arrange to clear it when it is done
    trap '__wezterm_set_user_var PROG ""' EXIT

    # and now run the corresponding command, taking care to avoid looping
    # with the alias definition
    command "$@"
}

alias vim="_run_prog vim"
alias tmux="_run_prog tmux"
alias nvim="_run_prog nvim"
```

Then on the wezterm side, this information can be used when formatting the tab titles:

```lua
local wezterm = require 'wezterm'

wezterm.on('format-tab-title', function(tab)
  local prog = tab.active_pane.user_vars.PROG
  return tab.active_pane.title .. ' [' .. (prog or '') .. ']'
end)

return {}
```

If you install the [wezterm shell integration](../shell-integration.md) you
will get a more comprehensive set of user vars set for you automatically.

User vars enable you to very deliberately signal information from your pane to
your wezterm config, and will work across multiplexer connections and even
through tmux (provided that you use the [tmux passthrough escape
sequence](https://github.com/tmux/tmux/wiki/FAQ#what-is-the-passthrough-escape-sequence-and-how-do-i-use-it)
to allow it to pass through).

The downside is that you need to take steps to ensure that your shell knows to
emit the appropriate user vars when you need them.

Depending on your needs, there are some alternative ways to reason about
specific information in a pane.

## OSC 0, 1, 2 for setting the Window/Pane Title

wezterm, like many other terminals, will interpret Operating System Command
(OSC) escape sequences for codes 0, 1 and 2 as updating the title:

|OSC|Description|Action|Example|
|---|-----------|------|-------|
|0  |Set Icon Name and Window Title | Clears Icon Name, sets Window Title. | `\x1b]0;title\x1b\\` |
|1  |Set Icon Name | Sets Icon Name, which is used as the Tab title when it is non-empty | `\x1b]1;tab-title\x1b\\` |
|2  |Set Window Title | Set Window Title | `\x1b]2;window-title\x1b\\` |

[pane:get_title()](../config/lua/pane/get_title.md) and/or the
[PaneInformation](../config/lua/PaneInformation.md) `title` field can be used
to retrieve the effective title that has been set for a pane.

It is common practice for shells in many distributions to arrange to set OSC 2
prior to executing a command. wezterm doesn't currently set this up
automatically. Note that wezterm will attempt to determine the foreground
process and substitute its title if the pane is a local pane and no title has
been set by an OSC escape sequence.

## OSC 7 for setting the current working directory

Emitting OSC 7 will tell wezterm to use a specific URI for the current working
directory associated with a pane:

```bash
printf "\033]7;file://HOSTNAME/CURRENT/DIR\033\\"
```

You may also use `wezterm set-working-directory` for this if you have `wezterm`
available.

The value you set via OSC 7 is available
[pane:get_current_working_dir()](../config/lua/pane/get_current_working_dir.md)
and/or the [PaneInformation](../config/lua/PaneInformation.md)
`current_working_dir` field can be used to retrieve the working directory that
has been set for a pane.  If OSC 7 has never been used in a pane, and that pane
is a local pane, wezterm can attempt to determine the working directory of the
foreground process that is associated with the pane.

Installing the [wezterm shell integration](../shell-integration.md) will
arrange for bash/zsh to set OSC 7 for you.

## Local Process State

wezterm provides some functions that can attempt to extract information about
processes that are running on the local machine; these will not work with
multiplexer connections of any kind (even unix multiplexers):

* [pane:get_foreground_process_info()](../config/lua/pane/get_foreground_process_info.md) -
  returns information about the process hierarchy in a pane
* [wezterm.procinfo.get_info_for_pid()](../config/lua/wezterm.procinfo/get_info_for_pid.md) -
  returns information about the process hierarchy for a given process id

There are a couple of other similar/related methods available to the
[Pane](../config/lua/pane/index.md) object and in the
[wezterm.procinfo](../config/lua/wezterm.procinfo/index.md) module.

Because these local process functions don't require changing your shell
configuration to get them working, they may be the most convenient to use in
your wezterm configuration, but they are limited to local processes only and
may not work as well to determine the correct foreground process when running
on Windows.
