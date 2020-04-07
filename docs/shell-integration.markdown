## Shell Integration

wezterm supports integrating with the shell through the following means:

### OSC 7 Escape sequence to set the working directory

`OSC` is escape sequence jargon for *Operating System Command*; `OSC 7` means
Operating System Command number 7.  This is an escape sequence that originated
in the macOS Terminal application that is used to advise the terminal of the
current working directory.

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

### OSC 7 on Windows with cmd.exe

`cmd.exe` doesn't allow a lot of flexibility in configuring the prompt,
but fortunately it does allow for emitting escape sequences.  You
can use the `set_environment_variables` configuration to pre-configure
the prompt environment in your `.wezterm.lua`; this example configures
the use of OSC 7 as well as including the time and current directory in
the visible prompt with green and purple colors, and makes the prompt
span multiple lines:

```lua
return {
  set_environment_variables = {
    prompt = "$E]7;file://localhost/$P$E\\$E[32m$T$E[0m $E[35m$P$E[36m$_$G$E[0m ",
  }
}
```

## Using clink on Windows Systems

[Clink](https://github.com/mridgers/clink) brings bash style line editing to
your Windows cmd.exe experience.  If you haven't installed clink to be the
global default on your system, you can configure wezterm to launch clink by
setting the `default_prog` configuration in your `.wezterm.lua`; for example,
if you have extracted clink to `c:\clink_0.4.9` you might configure this:

```lua
local wezterm = require 'wezterm';

local default_prog;
local set_environment_variables = {}

if wezterm.target_triple == "x86_64-pc-windows-msvc" then
  -- Use OSC 7 as per the above example
  set_environment_variables["prompt"] = "$E]7;file://localhost/$P$E\\$E[32m$T$E[0m $E[35m$P$E[36m$_$G$E[0m "
  -- use a more ls-like output format for dir
  set_environment_variables["DIRCMD"] = "/d"
  -- And inject clink into the command prompt
  default_prog = {"cmd.exe", "/s", "/k", "c:/clink_0.4.9/clink_x64.exe", "inject", "-q"}
end

return {
  default_prog = default_prog,
  set_environment_variables = set_environment_variables,
}
```

Now, rather than just running `cmd.exe` on its own, this will cause `cmd.exe`
to self-inject the clink line editor.
