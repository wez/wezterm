## SSH Connections

wezterm uses libssh2 to provide an integrated SSH client.  The
client can be used to make ad-hoc SSH connections to remote hosts
by invoking the client:

```bash
$ wezterm ssh wez@my.server
```

(checkout `wezterm ssh -h` for more options).

When invoked in this way, wezterm may prompt you for SSH authentication
and once a connection is established, open a new terminal window with
your requested command, or your shell if you didn't specify one.

Creating a new tab will create a new channel in your existing session
so you won't need to re-authenticate for additional tabs that you
create.

SSH sessions created in this way are non-persistent and all associated
tabs will die if your network connection is interrupted.

Take a look at [the multiplexing section](multiplexing.html) for an
alternative configuration that connects to a remote wezterm instance
and preserves your tabs.

*Since: 20210404-112810-b63a949d*

wezterm is now able to parse `~/.ssh/config` and `/etc/ssh/ssh_config`
and respects the following options:

* `IdentityFile`
* `Hostname`
* `User`
* `Port`
* `ProxyCommand`
* `Host` (including wildcard matching)
* `UserKnownHostsFile`
* `IdentitiesOnly`

All other options are parsed but have no effect.  Notably, neither `Match` or
`Include` will do anything.

*Since: nightly builds only:*

`Match` is now recognized but currently supports only single-phase (`final`,
`canonical` are not supported) configuration parsing for `Host` and
`LocalUser`.  `Exec` based matches are recognized but not supported.

### CLI Overrides

`wezterm ssh` CLI allows overriding config settings via the command line.  This
example shows how to specify the private key to use when connecting to
`some-host`:

```bash
wezterm ssh -oIdentityFile=/secret/id_ed25519 some-host
```
