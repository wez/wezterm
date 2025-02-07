wezterm supports integrating with the shell through the following means:

* `OSC 7` Escape sequences to advise the terminal of the working directory
* `OSC 133` Escape sequence to define Input, Output and Prompt zones
* `OSC 1337` Escape sequences to set user vars for tracking additional shell state

`OSC` is escape sequence jargon for *Operating System Command*.

These sequences enable some improved user experiences, such as being able
to spawn new panes, tabs and windows with the same current working directory
as the current pane, [jumping through the scrollback to the start of an earlier command](config/lua/keyassignment/ScrollToPrompt.md),
or [conveniently selecting the complete output from a command](config/lua/keyassignment/SelectTextAtMouseCursor.md).

In order for these features to be enabled, you will need to configure your
shell program to emit the escape sequences at the appropriate place.

You can find some [examples for various shells in the wezterm
repo](https://github.com/wezterm/wezterm/tree/main/assets/shell-integration).

To use this file to setup shell integration in wezterm with Bash or Zsh, you can
copy the file to your computer and source it via `. /path/to/file.sh` in your `.bashrc`
or `.zshrc`, or you can install it at `/etc/profile.d` on most unix systems.

Xonsh is supported via a [term-integrations](https://github.com/jnoortheen/xontrib-term-integrations) plugin.

Starting with version 20210314-114017-04b7cedd, the Fedora and Debian packages
automatically activate shell integration for Bash and Zsh.
Starting with version 20230320.124340.559cb7b0, the Arch Linux package
also automatically activate it.

If you're on another system, more information on how these escapes work
can be found below.

[Learn more about OSC 133 Semantic Prompt Escapes](https://gitlab.freedesktop.org/Per_Bothner/specifications/blob/master/proposals/semantic-prompts.md).

## User Vars

`OSC 1337` provides a means for setting *user vars*, which are somewhat similar
to environment variables, except that they are variables associated with a
given pane rather than a process.

Installing the wezterm shell integration will define the following user vars
by default:

* `WEZTERM_PROG` - the command line being executed by the shell
* `WEZTERM_USER` - holds the output from `id -un`; the current user name
* `WEZTERM_HOST` - holds the output from `hostname`; the hostname that the shell is running on
* `WEZTERM_IN_TMUX` - holds `1` if the shell is running inside tmux, `0` otherwise

If you are a tmux user, you must ensure that you have `set -g allow-passthrough on` set
in your tmux.conf for user vars to work.

Those vars will be updated each time the prompt is shown and just prior to executing a command.

The shell integration provides a shell function named `__wezterm_set_user_var` which can be
used to set your own user vars.

Setting a user var will generate events in the window that contains
the corresponding pane:

* [user-var-changed](config/lua/window-events/user-var-changed.md), which
  allows you to directly take action when a var is set/changed.
* [update-status](config/lua/window-events/update-status.md) which allows you to update left/right status items
* the title and tab bar area will then update and trigger any associated events as part of that update

You can access the complete set of user vars in a given pane by calling
[pane:get_user_vars()](config/lua/pane/get_user_vars.md), or by accessing
the `user_vars` field in a [PaneInformation](config/lua/PaneInformation.md)
struct.

You may wish to use this information to adjust what is shown in your tab titles
or in the status area.

## OSC 7 Escape sequence to set the working directory

`OSC 7` means Operating System Command number 7.  This is an escape sequence
that originated in the macOS Terminal application that is used to advise the
terminal of the current working directory.

An application (usually your shell) can be configured to emit this escape
sequence when the current directory changes, or just to emit it each time
it prints the prompt.

The current working directory can be specified as a URL like this:

```bash
printf "\033]7;file://HOSTNAME/CURRENT/DIR\033\\"
```

**When the current working directory has been set via OSC 7, spawning
a new tab will use the current working directory of the current tab,
so that you don't have to manually change the directory**.

If you are on a modern Fedora installation, the defaults for bash and
zsh source a `vte.sh` script that configures the shell to emit this
sequence.  On other systems you will likely need to configure this
for yourself.

## OSC 7 on Windows with cmd.exe

`cmd.exe` doesn't allow a lot of flexibility in configuring the prompt,
but fortunately it does allow for emitting escape sequences.  You
can use the `set_environment_variables` configuration to pre-configure
the prompt environment in your `.wezterm.lua`; this example configures
the use of OSC 7 as well as including the time and current directory in
the visible prompt with green and purple colors, and makes the prompt
span multiple lines:

```lua
config.set_environment_variables = {
  prompt = '$E]7;file://localhost/$P$E\\$E[32m$T$E[0m $E[35m$P$E[36m$_$G$E[0m ',
}
```

## OSC 7 on Windows with powershell

You can configure a custom prompt in powershell by creating/editing your
[powershell profile](https://docs.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_profiles?view=powershell-7.1)
and defining a function like this:

```powershell
function prompt {
    $p = $executionContext.SessionState.Path.CurrentLocation
    $osc7 = ""
    if ($p.Provider.Name -eq "FileSystem") {
        $ansi_escape = [char]27
        $provider_path = $p.ProviderPath -Replace "\\", "/"
        $osc7 = "$ansi_escape]7;file://${env:COMPUTERNAME}/${provider_path}${ansi_escape}\"
    }
    "${osc7}PS $p$('>' * ($nestedPromptLevel + 1)) ";
}
```

## OSC 7 on Windows with powershell (with starship)

When using [Starship](https://starship.rs/), since it has taken control of the prompt, hooking in to set
OSC 7 can be achieved this way instead:

```powershell
$prompt = ""
function Invoke-Starship-PreCommand {
    $current_location = $executionContext.SessionState.Path.CurrentLocation
    if ($current_location.Provider.Name -eq "FileSystem") {
        $ansi_escape = [char]27
        $provider_path = $current_location.ProviderPath -replace "\\", "/"
        $prompt = "$ansi_escape]7;file://${env:COMPUTERNAME}/${provider_path}$ansi_escape\"
    }
    $host.ui.Write($prompt)
}
```

## Using Clink on Windows Systems

[Clink](https://github.com/chrisant996/clink) brings bash style line editing,
completions and autosuggestions to your Windows cmd.exe experience. If you
haven't installed clink to be the global default on your system, you can
configure wezterm to launch clink by setting the `default_prog` configuration
in your `.wezterm.lua`; for example, if you have extracted clink to `c:\clink`
you might configure this:

```lua
local wezterm = require 'wezterm'
local config = {}

config.set_environment_variables = {}

if wezterm.target_triple == 'x86_64-pc-windows-msvc' then
  -- Use OSC 7 as per the above example
  config.set_environment_variables['prompt'] =
    '$E]7;file://localhost/$P$E\\$E[32m$T$E[0m $E[35m$P$E[36m$_$G$E[0m '
  -- use a more ls-like output format for dir
  config.set_environment_variables['DIRCMD'] = '/d'
  -- And inject clink into the command prompt
  config.default_prog =
    { 'cmd.exe', '/s', '/k', 'c:/clink/clink_x64.exe', 'inject', '-q' }
end

return config
```

Now, rather than just running `cmd.exe` on its own, this will cause `cmd.exe`
to self-inject the clink line editor.
