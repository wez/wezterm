!!! note
    *multiplexing is still a young feature and is evolving rapidly.  Your feedback is welcomed!*

## Multiplexing

The out-of-the-box experience with `wezterm` allows you to multiplex local tabs
and windows which will persist until they are closed.  With a little extra
configuration you can enable local terminal multiplexing with features similar
to those in [tmux](https://github.com/tmux/tmux/wiki) or [screen](https://en.wikipedia.org/wiki/GNU_Screen).

Multiplexing in `wezterm` is based around the concept of *multiplexing domains*;
a domain is a distinct set of windows and tabs.  When wezterm starts up it
creates a default *local domain* to manage the windows and tabs in the UI, but it
can also be configured to start or connect to additional domains.

Once connected to a domain, `wezterm` can attach its windows and tabs to the
local native UI, providing a more natural experience for interacting with
the mouse, clipboard and scrollback features of the terminal.

Key bindings allow you to spawn new tabs in the default local domain,
the domain of the current tab, or a specific numbered domain.

## SSH Domains

*wezterm also supports [regular ad-hoc ssh connections](ssh.md).
This section of the docs refers to running a wezterm daemon on the remote end
of a multiplexing session that uses ssh as a channel*

A connection to a remote wezterm multiplexer made via an ssh connection is
referred to as an *SSH domain*.  **A compatible version of wezterm must be
installed on the remote system in order to use SSH domains**.
SSH domains are supported on all systems via libssh2.

To configure an SSH domain, place something like the following in
your `.wezterm.lua` file:

```lua
config.ssh_domains = {
  {
    -- This name identifies the domain
    name = 'my.server',
    -- The hostname or address to connect to. Will be used to match settings
    -- from your ssh config file
    remote_address = '192.168.1.1',
    -- The username to use on the remote host
    username = 'wez',
  },
}
```

[See SshDomain](config/lua/SshDomain.md) for more information on possible
settings to use with SSH domains.

To connect to the system, run:

```console
$ wezterm connect my.server
```

This will launch an SSH session that connects to the specified address
and may pop up authentication dialogs (using SSH keys for auth is
strongly recommended!).  Once connected, it will attempt to spawn
the wezterm multiplexer daemon on the remote host and connect to
it via a unix domain socket using a similar mechanism to that
described in the *Unix Domains* section below.

{{since('20230408-112425-69ae8472')}}

Ssh_domains now auto-populate from your `~/.ssh/config` file. Each populated host will have both a plain SSH and a multiplexing SSH domain. Plain SSH hosts are defined with a `SSH:` prefix to their name and multiplexing hosts are defined with a prefix `SSHMUX:`. For example, to connect to a host named `my.server` in your `~/.ssh/config` using a multiplexing domain, run:

```console
$ wezterm connect SSHMUX:my.server
# or to spawn into a new tab in an existing wezterm gui instance:
$ wezterm cli spawn --domain-name SSHMUX:my.server
```

To customize this functionality, see the example for [wezterm.default_ssh_domains()](config/lua/wezterm/default_ssh_domains.md)

## Unix Domains

A connection to a multiplexer made via a unix socket is referred to
as a *unix domain*.  Unix domains are supported on all systems,
[even Windows](https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/)
and are a way to connect the native win32 GUI into the Windows Subsystem for Linux (WSL).

The bare minimum configuration to enable a unix domain is this, which will
spawn a server if needed and then connect the gui to it automatically
when wezterm is launched:

```lua
config.unix_domains = {
  {
    name = 'unix',
  },
}

-- This causes `wezterm` to act as though it was started as
-- `wezterm connect unix` by default, connecting to the unix
-- domain on startup.
-- If you prefer to connect manually, leave out this line.
config.default_gui_startup_args = { 'connect', 'unix' }
```

If you prefer to connect manually, omit the `default_gui_startup_args` setting
and then run:

```console
$ wezterm connect unix
```

Note that in earlier versions of wezterm, a `connect_automatically` domain
option was shown as the way to connect on startup.  Using
`default_gui_startup_args` is recommended instead as it works more reliably.

The possible configuration values are:

```lua
config.unix_domains = {
  {
    -- The name; must be unique amongst all domains
    name = 'unix',

    -- The path to the socket.  If unspecified, a reasonable default
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
  },
}
```

{{since('20220101-133340-7edc5b5a')}}

It is now possible to specify a `proxy_command` that will be used
in place of making a direct unix connection.  When `proxy_command`
is specified, it will be used instead of the optional `socket_path`.

This example shows a redundant use of `nc` (netcat) to connect to
the unix socket path on my mac.  This isn't useful on its own,
but may help with the WSL 2 issue mentioned below when translated
to an appropriate invocation of netcat/socat on Windows:

```lua
config.unix_domains = {
  {
    name = 'unix',
    proxy_command = { 'nc', '-U', '/Users/wez/.local/share/wezterm/sock' },
  },
}
```

{{since('20220319-142410-0fcdea07')}}

You may now specify the round-trip latency threshold for enabling predictive
local echo using `local_echo_threshold_ms`. If the measured round-trip latency
between the wezterm client and the server exceeds the specified threshold, the
client will attempt to predict the server's response to key events and echo the
result of that prediction locally without waiting, hence hiding latency to the
user. This option only applies when `multiplexing = "WezTerm"`.

```lua
config.unix_domains = {
  {
    name = 'unix',
    local_echo_threshold_ms = 10,
  },
}
```

### Connecting into Windows Subsystem for Linux

*Note: this only works with WSL 1. [WSL 2 doesn't support AF_UNIX interop](https://github.com/microsoft/WSL/issues/5961)*

Inside your WSL instance, configure `.wezterm.lua` with this snippet:

```lua
config.unix_domains = {
  {
    name = 'wsl',
    -- Override the default path to match the default on the host win32
    -- filesystem.  This will allow the host to connect into the WSL
    -- container.
    socket_path = '/mnt/c/Users/USERNAME/.local/share/wezterm/sock',
    -- NTFS permissions will always be "wrong", so skip that check
    skip_permissions_check = true,
  },
}
```

In the host win32 configuration, use this snippet:

```lua
config.unix_domains = {
  {
    name = 'wsl',
    serve_command = { 'wsl', 'wezterm-mux-server', '--daemonize' },
  },
}
config.default_gui_startup_args = { 'connect', 'wsl' }
```

Now when you start wezterm you'll be presented with a WSL tab.

You can also omit `default_gui_startup_args` and use:

```console
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
config.tls_clients = {
  {
    -- A handy alias for this session; you will use `wezterm connect server.name`
    -- to connect to it.
    name = 'server.name',
    -- The host:port for the remote host
    remote_address = 'server.hostname:8080',
    -- The value can be "user@host:port"; it accepts the same syntax as the
    -- `wezterm ssh` subcommand.
    bootstrap_via_ssh = 'server.hostname',
  },
}
```

[See TlsDomainClient](config/lua/TlsDomainClient.md) for more information on possible
settings.

### Configuring the server

```lua
config.tls_servers = {
  {
    -- The host:port combination on which the server will listen
    -- for connections
    bind_address = 'server.hostname:8080',
  },
}
```

[See TlsDomainServer](config/lua/TlsDomainServer.md) for more information on possible
settings.

### Connecting

On the client, running this will connect to the server, start up
the multiplexer and obtain a certificate for the TLS connection.
A connection window will show the progress and may prompt you for
SSH authentication.  Once the connection has been initiated, wezterm
will automatically reconnect using the certificate it obtained during
bootstrapping if your connection was interrupted and resume your
remote terminal session

```console
$ wezterm connect server.name
```
