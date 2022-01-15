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

*Since: 20220101-133340-7edc5b5a*

You may now specify a table with ssh config overrides:

```lua
return {
  ssh_domains = {
    {
      name = "my.server",
      remote_address = "192.168.1.1",
      ssh_option = {
        identityfile = "/path/to/id_rsa.pub",
      }
    }
  }
}
```

*Since: nightly builds only*

You may now specify the type of `multiplexing` used by an ssh domain.
The following values are possible:

* `"WezTerm"` - this is the default; use wezterm's multiplexing client.
  Having wezterm installed on the server is required to use this mode.
* `"None"` - don't use any multiplexing. The connection is an ssh connection
  using the same mechanism as is used by `wezterm ssh`; losing connectivity
  will lose any panes/tabs.  This mode of operation is convenient when using
  SSH to connect automatically into eg: a locally hosted WSL instance, together
  with the [default_domain](config/default_domain.md) option.

A new `assume_shell` option, when coupled with `multiplexing = "None"`, allows
wezterm to assume that the remote host uses a specific shell command language
dialect, which in turn allows wezterm to respect the
current working directory as set by [OSC 7 / Shell
Integration](../../shell-integration.md) on the remote host when spawning new
panes and tabs.  The following values are recognized for `assume_shell`:

* `"Unknown"` - this is the default. We can't make any assumptions about the
  remote shell.
* `"Posix"` - the remote host uses a Bourne Shell compatible shell that allows
  the syntax `cd DIR ; exec CMD` and `cd DIR ; exec $SHELL`.


```lua
return {
  ssh_domains = {
    {
      name = "my.server",
      remote_address = "192.168.1.1",
      multiplexing = "None",

      -- When multiplexing == "None", default_prog can be used
      -- to specify the default program to run in new tabs/panes.
      -- Due to the way that ssh works, you cannot specify default_cwd,
      -- but you could instead change your default_prog to put you
      -- in a specific directory.
      default_prog = {"fish"},

      -- assume that we can use syntax like:
      -- "cd /some/where ; exec $SHELL"
      -- using whatever the default command shell is on this
      -- remote host, so that shell integration will respect
      -- the current directory on the remote host.
      assume_shell = "Posix",
    }
  },

  default_domain = "my.server",
}
```

