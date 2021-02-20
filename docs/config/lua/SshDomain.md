# SshDomain

The `SshDomain` struct specifies information about an individual
[SSH Domain](../../multiplexing.md#ssh-domains).

It is a lua object with the following fields:

```lua
{
    -- The name of this specific domain.  Must be unique amongst
    -- all types of domain in the configuration file.
    name = "my.server",

    -- identifies the host:port pair of the remote server
    -- Can be a DNS name or an IP address with an optional
    -- ":port" on the end.
    remote_address = "192.168.1.1",

    -- Whether agent auth should be disabled.
    -- Set to true to disable it.
    -- no_agent_auth = false,

    -- The username to use for authenticating with the remote host
    username = "yourusername",

    -- If true, connect to this domain automatically at startup
    -- connect_automatically = true,

    -- Specify an alternative read timeout
    -- timeout = 60,

    -- The path to the wezterm binary on the remote host.
    -- Primarily useful if it isn't installed in the $PATH
    -- that is configure for ssh.
    -- remote_wezterm_path = "/home/yourusername/bin/wezterm"
}
```
