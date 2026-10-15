---
tags:
  - multiplexing
---
# `exec_domains`

{{since('20220807-113146-c2fee766')}}

Defines a list of [ExecDomain](../ExecDomain.md) objects. These are local
multiplexer domains that can adjust a command before it is executed, for
example to run it through a wrapper program.

The default is an empty list, so no execution domains are configured.

Create each entry with `wezterm.exec_domain(name, fixup, label)`. The `name`
must be unique among all multiplexer domains. The `fixup` callback receives a
[SpawnCommand](../SpawnCommand.md) and must return the adjusted command. The
`label` argument is optional.

```lua
local wezterm = require 'wezterm'
local config = {}

config.exec_domains = {
  wezterm.exec_domain('mydomain', function(cmd)
    -- Adjust cmd here to wrap the requested command.
    return cmd
  end),
}

return config
```

To use an execution domain when starting the GUI, set
[default_domain](default_domain.md) to its name.

See [ExecDomain](../ExecDomain.md) for callback details and examples that run
commands in Docker containers or systemd scopes.
