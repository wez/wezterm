---
title: wezterm.default_ssh_domains
tags:
 - ssh
 - multiplexing
---

# wezterm.default_ssh_domains()

{{since('20230408-112425-69ae8472')}}

Computes a list of [SshDomain](../SshDomain.md) objects based on
the set of hosts discovered in your `~/.ssh/config`.

Each host will have both a plain SSH and a multiplexing SSH domain
generated and returned in the list of domains.  The former don't
require wezterm to be installed on the remote host, while the
latter do require it.

The intended purpose of this function is to allow you the opportunity
to edit/adjust the returned information before assigning it to
your config.

For example, if all of the hosts referenced by your ssh config
are unix machines, you might want to inform wezterm of that
so that features like spawning a tab in the same directory
as an existing tab work even for a plain SSH session:

```lua
config.ssh_domains = wezterm.default_ssh_domains()
for _, dom in ipairs(config.ssh_domains) do
  dom.assume_shell = 'Posix'
end
```

