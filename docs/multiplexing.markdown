**Notice:** *multiplexing is still a young feature and is evolving rapidly.
Your feedback is welcomed!*

## Multiplexing

The out-of-the-box experience with `wezterm` allows you to multiplex local tabs
and windows which will persist until they are closed.  With a little extra
configuration you can enable local terminal multiplexing with features similar
to those in [tmux](https://github.com/tmux/tmux/wiki) or [screen](https://en.wikipedia.org/wiki/GNU_Screen).

Multiplexing in `wezterm` is based around the concept of *multiplexing domains*;
a domain is a distinct set of windows and tabs.  When wezterm starts up it
creates a default *local domain* to manage the windows and tabs in the UI, but it
can also be configured to start or connect to any number of additional domains.

Once connected to a domain, `wezterm` can attach its windows and tabs to the
local native UI, providing a more natural experience for interacting with
the mouse, clipboard and scrollback features of the terminal.

Key bindings allow you to spawn new tabs in the default local domain,
the domain of the current tab, or a specific numbered domain.

## SSH Domains

*wezterm also supports [regular ad-hoc ssh connections](ssh.html).
This section of the docs refers to running a wezterm daemon on the remote end
of a multiplexing session that uses ssh as a channel*

A connection to a remote wezterm multiplexer made via an ssh connection is
referred to as an *SSH domain*.  **A compatible version of wezterm must be
installed on the remote system in order to use SSH domains**.
SSH domains are supported on all systems via libssh2.

To configure an SSH domain, place something like the following in
your `.wezterm.lua` file:

```lua
return {
  ssh_domains = {
    {
      -- This name identifies the domain
      name = "my.server",
      -- The address to connect to
      remote_address = "192.168.1.1",
      -- The username to use on the remote host
      username = "wez",
    }
  }
}
```

To connect to the system, run:

```bash
$ wezterm connect my.server
```

This will launch an SSH session that connects to the specified address
and may pop up authentication dialogs (using SSH keys for auth is
strongly recommended!).  Once connected, it will attempt to spawn
the wezterm multiplexer daemon on the remote host and connect to
it via a unix domain socket using a similar mechanism to that
described in the *Unix Domains* section below.

## Unix Domains

A connection to a multiplexer made via a unix socket is referred to
as a *unix domain*.  Unix domains are supported on all systems,
[even Windows](https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/)
and are a way to connect the native win32 GUI into the Windows Subsystem for Linux (WSL).

The bare minimum configuration to enable a unix domain is this, which will
spawn a server if needed and then connect the gui to it automatically
when wezterm is launched:

```lua
return {
  unix_domains = {
    {
      name = "unix",
      connect_automatically = true,
    }
  }
}
```

If you prefer to connect manually, omit the `connect_automatically` setting
(or set it to `false`) and then run:

```bash
$ wezterm connect unix
```

The possible configuration values are:

```lua
return {
  unix_domains = {
    {
      name = "unix",
      -- If true, connect to this unix domain when `wezterm` is started
      connect_automatically = true,

      -- The path to the socket.  If unspecified, a resonable default
      -- value will be computed.

      -- socket_path = "/some/path",

      -- If true, do not attempt to start this server if we try and fail to
      -- connect to it.

      -- no_serve_automatically = false,

      -- If true, bypass checking for secure ownership of the
      -- socket_path.  This is not recommended on a multi-user
      -- system, but is useful for example when running the
      -- server inside a WSL container but with the socket
      -- on the host NTFS volume.

      -- skip_permissions_check = false,

    }
  }
}
```

### Connecting into Windows Subsystem for Linux

Inside your WSL instance, configure `.wezterm.lua` with this snippet:

```lua
return {
  unix_domains = {
    {
      name = "wsl"
      -- Override the default path to match the default on the host win32
      -- filesystem.  This will allow the host to connect into the WSL
      -- container.
      socket_path = "/mnt/c/Users/USERNAME/.local/share/wezterm/sock",
      -- NTFS permissions will always be "wrong", so skip that check
      skip_permissions_check = true,
    }
  }
}
```

In the host win32 configuration, use this snippet:

```lua
return {
  unix_domains = {
    {
      name = "wsl",
      connect_automatically = true,
      serve_command = ["wsl", "wezterm", "start", "--daemonize", "--front-end", "MuxServer"],
      -- NOTE: nightly builds use this instead:
      serve_command = ["wsl", "wezterm-mux-server", "--daemonize"],
    }
  }
}
```

Now when you start wezterm you'll be presented with a WSL tab.

You can also set `connect_automatically = false` and use:

```bash
$ wezterm connect wsl
```

to manually connect into your WSL instance.

## TLS Domains

A connection to a multiplexer made via a [TLS](https://en.wikipedia.org/wiki/Transport_Layer_Security)
encrypted TCP connection is referred to as a *TLS Domain*.

Starting with version `20200202-180558-2489abf9`, wezterm can bootstrap a TLS
session by performing an initial connection via SSH to start the wezterm
multiplexer on the remote host and securely obtain a key.  Once bootstrapped,
the client will use a TLS protected TCP connection to communicate with the
server.

### Configuring the client

For each server that you wish to connect to, add a client section like this:

```lua
return {
  tls_clients = {
    {
      -- A handy alias for this session; you will use `wezterm connect server.name`
      -- to connect to it.
      name = "server.name",
      -- The host:port for the remote host
      remote_address = "server.hostname:8080",
      -- The value can be "user@host:port"; it accepts the same syntax as the
      -- `wezterm ssh` subcommand.
      bootstrap_via_ssh = "server.hostname",
    }
  }
}
```

### Configuring the server

```lua
return {
  tls_servers = {
    {
      -- The host:port combination on which the server will listen
      -- for connections
      bind_address = "server.hostname:8080"
    }
  }
}
```

### Connecting

On the client, running this will connect to the server, start up
the multiplexer and obtain a certificate for the TLS connection.
A connection window will show the progress and may prompt you for
SSH authentication.  Once the connection has been initiated, wezterm
will automatically reconnect using the certificate it obtained during
bootstrapping if your connection was interrupted and resume your
remote terminal session

```bash
$ wezterm connect server.name
```
