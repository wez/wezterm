---
title: wezterm.enumerate_ssh_hosts
tags:
 - ssh
---

# wezterm.enumerate_ssh_hosts(\[ssh_config_file_name, ...\])

{{since('20220319-142410-0fcdea07')}}

This function will parse your ssh configuration file(s) and extract from them
the set of literal (non-pattern, non-negated) host names that are specified in
`Host` and `Match` stanzas contained in those configuration files and return a
mapping from the hostname to the effective ssh config options for that host.

You may optionally pass a list of ssh configuration files that should be read,
in case you have a special configuration.

The files you specify (if any) will be parsed first, and then the default
locations for your system will be parsed.

All files read by a call to this function, and any `include` statements
processed from those ssh config files, will be added to the config reload watch
list as though
[wezterm.add_to_config_reload_watch_list()](add_to_config_reload_watch_list.md)
was called on them.  Note that only concrete path names are watched: if your
config uses `include` to include glob patterns in a directory then, for
example, newly created files in that directory will not cause a config reload
event in wezterm.

This example shows how to use this function to automatically configure ssh
multiplexing domains for the hosts configured in your `~/.ssh/config` file:

```lua
local wezterm = require 'wezterm'

local ssh_domains = {}

for host, config in pairs(wezterm.enumerate_ssh_hosts()) do
  table.insert(ssh_domains, {
    -- the name can be anything you want; we're just using the hostname
    name = host,
    -- remote_address must be set to `host` for the ssh config to apply to it
    remote_address = host,

    -- if you don't have wezterm's mux server installed on the remote
    -- host, you may wish to set multiplexing = "None" to use a direct
    -- ssh connection that supports multiple panes/tabs which will close
    -- when the connection is dropped.

    -- multiplexing = "None",

    -- if you know that the remote host has a posix/unix environment,
    -- setting assume_shell = "Posix" will result in new panes respecting
    -- the remote current directory when multiplexing = "None".
    assume_shell = 'Posix',
  })
end

return {
  ssh_domains = ssh_domains,
}
```

This shows the structure of the returned data, by evaluating the function in the [debug overlay](../keyassignment/ShowDebugOverlay.md) (`CTRL-SHIFT-L`):

```
> wezterm.enumerate_ssh_hosts()
{
    "aur.archlinux.org": {
        "hostname": "aur.archlinux.org",
        "identityagent": "/run/user/1000/keyring/ssh",
        "identityfile": "/home/wez/.ssh/aur",
        "port": "22",
        "user": "aur",
        "userknownhostsfile": "/home/wez/.ssh/known_hosts /home/wez/.ssh/known_hosts2",
    },
    "woot": {
        "hostname": "localhost",
        "identityagent": "/run/user/1000/keyring/ssh",
        "identityfile": "/home/wez/.ssh/id_dsa /home/wez/.ssh/id_ecdsa /home/wez/.ssh/id_ed25519 /home/wez/.ssh/id_rsa",
        "port": "22",
        "user": "someone",
        "userknownhostsfile": "/home/wez/.ssh/known_hosts /home/wez/.ssh/known_hosts2",
    },
}
```

the corresponding `~/.ssh/config` file for the above is shown below: note host
the `Host` group with a wildcard is not returned by the function because it
doesn't have a concrete host name:

```
Host aur.archlinux.org
  IdentityFile ~/.ssh/aur
  User aur

Host 192.168.1.*
  ForwardAgent yes
  ForwardX11 yes

Host woot
  User someone
  Hostname localhost
```
