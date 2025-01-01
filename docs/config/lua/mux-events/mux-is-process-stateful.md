# `mux-is-process-stateful`

{{since('20220101-133340-7edc5b5a')}}

The `mux-is-process-stateful` event is emitted when the multiplexer layer wants
to determine whether a given Pane can be closed without prompting the user.

This event is *synchronous* and must return as quickly as possible in order
to avoid blocking the multiplexer.

The event is passed a [LocalProcessInfo](../LocalProcessInfo.md) object
representing the process that corresponds to the pane.

The hook can return one of the following values:

* `true` - to indicate that this process tree is considered to be stateful and that the user should be prompted before terminating the pane
* `false` - to indicate that the process tree can be terminated *without* prompting the user
* `nil` - to use the default behavior, which is to consider the [skip_close_confirmation_for_processes_named](../config/skip_close_confirmation_for_processes_named.md) configuration option
* any other value, or an error, will be treated as equivalent to returning `nil`

## Example

This example doesn't change any behavior, but demonstrates how to log the various fields of process tree,
indenting the entries for each level of the process hierarchy.

Since it returns `nil`, it uses the default behavior.

```lua
local wezterm = require 'wezterm'

function log_proc(proc, indent)
  indent = indent or ''
  wezterm.log_info(
    indent
      .. 'pid='
      .. proc.pid
      .. ', name='
      .. proc.name
      .. ', status='
      .. proc.status
  )
  wezterm.log_info(indent .. 'argv=' .. table.concat(proc.argv, ' '))
  wezterm.log_info(
    indent .. 'executable=' .. proc.executable .. ', cwd=' .. proc.cwd
  )
  for pid, child in pairs(proc.children) do
    log_proc(child, indent .. '  ')
  end
end

wezterm.on('mux-is-process-stateful', function(proc)
  log_proc(proc)

  -- Just use the default behavior
  return nil
end)

return {}
```

Produces the following logs for a `zsh` that spawned `bash` that spawned `vim foo`:

```
INFO  config::lua > lua: pid=1913470, name=zsh, status=Sleep
INFO  config::lua > lua: argv=-zsh
INFO  config::lua > lua: executable=/usr/bin/zsh, cwd=/home/wez
INFO  config::lua > lua:   pid=1913567, name=bash, status=Sleep
INFO  config::lua > lua:   argv=bash
INFO  config::lua > lua:   executable=/usr/bin/bash, cwd=/home/wez
INFO  config::lua > lua:     pid=1913624, name=vim, status=Sleep
INFO  config::lua > lua:     argv=vim foo
INFO  config::lua > lua:     executable=/usr/bin/vim, cwd=/home/wez
```
