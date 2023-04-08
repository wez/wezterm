---
tags:
  - ssh
  - multiplexing
---
# `ssh_domains`

Configures SSH multiplexing domains.  [Read more about SSH Domains](
../../../multiplexing.md#ssh-domains).

This option accepts a list of [SshDomain](../SshDomain.md) objects.

{{since('20230408-112425-69ae8472')}}

If you don't set `ssh_domains` in your config, wezterm will default
to configuring it as if you had:

```lua
config.ssh_domains = wezterm.default_ssh_domains()
```

See also [wezterm.default_ssh_domains()](../wezterm/default_ssh_domains.md).

