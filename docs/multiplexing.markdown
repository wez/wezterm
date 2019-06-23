---
title: Multiplexing
---

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

## Unix Domains

A connection to a multiplexer made via a unix socket is referred to
as a *unix domain*.  Unix domains are supported on all systems,
[even Windows](https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/)
and are a way to connect the native win32 GUI into the Windows Subsystem for Linux (WSL).

The bare minimum configuration to enable a unix domain is this, which will
spawn a server if needed and then connect the gui to it automatically
when wezterm is launched:

```toml
[[unix_domains]]
connect_automatically = true
```

The possible configuration values are:

```toml
[[unix_domains]]
# If true, connect to this unix domain when `wezterm` is started
connect_automatically = true

# The path to the socket.  If unspecified, a resonable default
# value will be computed.
# socket_path = "/some/path"

# If true, do not attempt to start this server if we try and fail to
# connect to it.
# no_serve_automatically = false

# If true, bypass checking for secure ownership of the
# socket_path.  This is not recommended on a multi-user
# system, but is useful for example when running the
# server inside a WSL container but with the socket
# on the host NTFS volume.
# skip_permissions_check = false
```

### Connecting into Windows Subsystem for Linux

Inside your WSL instance, configure `wezterm.toml` with this snippet:

```toml
[[unix_domains]]
# Override the default path to match the default on the host win32
# filesystem.  This will allow the host to connect into the WSL
# container.
socket_path = "/mnt/c/Users/USERNAME/.local/share/wezterm/sock"
# NTFS permissions will always be "wrong", so skip that check
skip_permissions_check = true
```

and then start the server:

```bash
$ wezterm start --front-end MuxServer --daemonize
```

In the host win32 configuration, use this snippet:

```toml
[[unix_domains]]
connect_automatically = true
```

Now when you start wezterm you'll be presented with a WSL tab.

## TLS Domains

**Notice:** *TLS domains require external PKI infrastructure to authenticate
both the client and the server side with each other. wezterm doesn't
provide an easy way to manage this at this time*.

A connection to a multiplexer made via a [TLS](https://en.wikipedia.org/wiki/Transport_Layer_Security)
encrypted TCP connection is referred to as a *TLS Domain*.  Configuring
a TLS Domain is currently a bit awkward and requires mutual certificate-based
authentication of both ends of the connection.  There are no instructions
on how to set up the certificates at this time, but this will be expanded
as the user experience around this feature is fleshed out.

### Requirements

You provide a PKI infrastructure that can generate:

  * A certificate for the host with the CN set to the hostname
  * A certificate for the client with the CN set to the unixname
    of the connecting user.  The server MUST also be running with
    the `$USER` environment variable set to that unixname.
  * The CA/chain of certificates must be available to verify those
    certificates on both sides of the TLS session.
  * **Guard the user certificate and key carefully** as it is the sole
    means of authenticating the client and will allow execution of arbitrary
    commands on the server as that user.

### Configuring the client

For each server that you wish to connect to, add a client section like this:

```toml
[[tls_clients]]
# The host:port for the remote host
remote_address = "server.hostname:8080"
# The client private key for your user.  Guard this carefully as
# posession of this secret allows executing commands as you on
# the remote host!  The subject COMMONNAME (or CN) must match
# the USER environment variable that the server side runs as
# otherwise the connection will be rejected.
pem_private_key = "/secure/wez.key"
# The public certificate that corresponds to `pem_private_key`
pem_cert = "/secure/wez.pem"

# A CA file or bundle to help verify certificates
pem_ca = "/secure/ca.pem"
# A list of CA files or directories containing CA files that will
# also be used to verify certificates.
pem_root_certs = ["/secure/trusted-certs"]
connect_automatically = true
```

### Configuring the server

```toml
[[tls_servers]]
# The host:port combination on which the server will listen
# for connections
bind_address = "server.hostname:8080"
# The server private key
pem_private_key = "/secure/server.key"
# The public certificate that corresponds to the `pem_private_key`.
# The subject COMMONNAME (or CN) must match the hostname used
# by the client to connect to the server, or the client will
# refuse to connect.
pem_cert = "/secure/server.pem"

# A CA file or bundle to help verify certificates
pem_ca = "/secure/ca.pem"
# A list of CA files or directories containing CA files that will
# also be used to verify certificates.
pem_root_certs = ["/secure/trusted-certs"]
```

### Starting the server

At this time, `wezterm` doesn't provide a convenient way to automatically
start the server, so you will need to manually log in to the server host
and start it up:

```bash
$ wezterm start --front-end MuxServer --daemonize
```
