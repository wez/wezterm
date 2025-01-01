# ExecDomain

{{since('20220807-113146-c2fee766')}}

An `ExecDomain` defines a local-execution multiplexer domain. In simple terms,
rather than directly executing the requested program, an `ExecDomain` allows
you wrap up that command invocation by passing it through some other process.

For example, if you wanted to make it more convenient to work with tabs and
panes inside a docker container, you might want to define an `ExecDomain` that
causes the commands to be run via `docker exec`.  While you could just ask
wezterm to explicitly spawn a command that runs `docker exec`, you would also
need to adjust the default key assignments for splitting panes to know about
that preference.  Using an `ExecDomain` allows that preference to be associated
with the pane so that things work more intuitively.

## Defining an ExecDomain

You must use the `wezterm.exec_domain` function to define a domain. It accepts
the following parameters:

```
wezterm.exec_domain(NAME, FIXUP [, LABEL])
```

* *name* - uniquely identifies the domain. Must be different from any other multiplexer domains.
* *fixup* - a lua function that will be called to *fixup* the requested command and return the revised command
* *label* - optional. Can be either a string to serve as a label in the
  [Launcher Menu](../launch.md#the-launcher-menu), or a lua function that will
  return the label.

### fixup

The simplest fixup function looks like this:

```lua
wezterm.exec_domain('myname', function(cmd)
  return cmd
end)
```

The *cmd* parameter is a [SpawnCommand](SpawnCommand.md) that contains
information about the command that is to be executed. This will either be
something that the user configured as a key assignment or will be an equivalent
generated in response to a request to spawn a new tab or split a pane.

It is expected that your fixup function will adjust the various fields
of the provided command and then return it.  The adjusted command is
what wezterm will execute in order to satisfy the user's request to
spawn a new program.

### label

The label is visible in the [Launcher Menu](../launch.md#the-launcher-menu).
You may set it a static string or set it to a lua callback.  The default
behavior is equivalent to this callback function definition:

```lua
-- domain_name is the same name you used as the first parameter to
-- wezterm.exec_domain()
wezterm.exec_domains(domain_name, fixup_func, function(domain_name)
  return domain_name
end)
```

Using a callback function allows you to produce an amended label
just in time for the launcher menu to be rendered. That is useful
for example to adjust it to represent some status information.
If you were defining an ExecDomain for a docker container or
VM, then you could have the label reflect whether it is currently
running.

Both the static string and the generated string may include escape sequences
that affect the styling of the text. You may wish to use
[wezterm.format()](wezterm/format.md) to manage that.

## Example: Running commands in their own systemd scope

```lua
local wezterm = require 'wezterm'
local config = {}

-- Equivalent to POSIX basename(3)
-- Given "/foo/bar" returns "bar"
-- Given "c:\\foo\\bar" returns "bar"
local function basename(s)
  return string.gsub(s, '(.*[/\\])(.*)', '%2')
end

config.exec_domains = {
  -- Defines a domain called "scoped" that will run the requested
  -- command inside its own individual systemd scope.
  -- This defines a strong boundary for resource control and can
  -- help to avoid OOMs in one pane causing other panes to be
  -- killed.
  wezterm.exec_domain('scoped', function(cmd)
    -- The "cmd" parameter is a SpawnCommand object.
    -- You can log it to see what's inside:
    wezterm.log_info(cmd)

    -- Synthesize a human understandable scope name that is
    -- (reasonably) unique. WEZTERM_PANE is the pane id that
    -- will be used for the newly spawned pane.
    -- WEZTERM_UNIX_SOCKET is associated with the wezterm
    -- process id.
    local env = cmd.set_environment_variables
    local ident = 'wezterm-pane-'
      .. env.WEZTERM_PANE
      .. '-on-'
      .. basename(env.WEZTERM_UNIX_SOCKET)

    -- Generate a new argument array that will launch a
    -- program via systemd-run
    local wrapped = {
      '/usr/bin/systemd-run',
      '--user',
      '--scope',
      '--description=Shell started by wezterm',
      '--same-dir',
      '--collect',
      '--unit=' .. ident,
    }

    -- Append the requested command
    -- Note that cmd.args may be nil; that indicates that the
    -- default program should be used. Here we're using the
    -- shell defined by the SHELL environment variable.
    for _, arg in ipairs(cmd.args or { os.getenv 'SHELL' }) do
      table.insert(wrapped, arg)
    end

    -- replace the requested argument array with our new one
    cmd.args = wrapped

    -- and return the SpawnCommand that we want to execute
    return cmd
  end),
}

-- Making the domain the default means that every pane/tab/window
-- spawned by wezterm will have its own scope
config.default_domain = 'scoped'

return config
```

## Example: docker domains

This example shows how to add each running docker container as a domain,
so that you can spawn a shell into it and/or split it:

{% raw %}
```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

function docker_list()
  local docker_list = {}
  local success, stdout, stderr = wezterm.run_child_process {
    'docker',
    'container',
    'ls',
    '--format',
    '{{.ID}}:{{.Names}}',
  }
  for _, line in ipairs(wezterm.split_by_newlines(stdout)) do
    local id, name = line:match '(.-):(.+)'
    if id and name then
      docker_list[id] = name
    end
  end
  return docker_list
end

function make_docker_label_func(id)
  return function(name)
    local success, stdout, stderr = wezterm.run_child_process {
      'docker',
      'inspect',
      '--format',
      '{{.State.Running}}',
      id,
    }
    local running = stdout == 'true\n'
    local color = running and 'Green' or 'Red'
    return wezterm.format {
      { Foreground = { AnsiColor = color } },
      { Text = 'docker container named ' .. name },
    }
  end
end

function make_docker_fixup_func(id)
  return function(cmd)
    cmd.args = cmd.args or { '/bin/sh' }
    local wrapped = {
      'docker',
      'exec',
      '-it',
      id,
    }
    for _, arg in ipairs(cmd.args) do
      table.insert(wrapped, arg)
    end

    cmd.args = wrapped
    return cmd
  end
end

function compute_exec_domains()
  local exec_domains = {}
  for id, name in pairs(docker_list()) do
    table.insert(
      exec_domains,
      wezterm.exec_domain(
        'docker:' .. name,
        make_docker_fixup_func(id),
        make_docker_label_func(id)
      )
    )
  end
  return exec_domains
end

config.exec_domains = compute_exec_domains()

return config
```
{% endraw %}

With something like the config above, each time the config is reloaded, the
list of available domains will be updated.

Opening the launcher menu will show them and their status and allow you
to launch programs inside those containers.

