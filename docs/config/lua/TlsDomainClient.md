# TlsDomainClient

The `TlsDomainClient` struct specifies information about how to connect
to a [TLS Domain](../../multiplexing.md#tls-domains).

It is a lua object with the following fields:

```lua
config.tls_clients = {
  {
    -- The name of this specific domain.  Must be unique amongst
    -- all types of domain in the configuration file.
    name = 'server.name',

    -- If set, use ssh to connect, start the server, and obtain
    -- a certificate.
    -- The value is "user@host:port", just like "wezterm ssh" accepts.
    bootstrap_via_ssh = 'server.hostname',

    -- identifies the host:port pair of the remote server.
    remote_address = 'server.hostname:8080',

    -- the path to an x509 PEM encoded private key file.
    -- Omit this if you are using `bootstrap_via_ssh`.
    -- pem_private_key = "/some/path/key.pem",

    -- the path to an x509 PEM encoded certificate file
    -- Omit this if you are using `bootstrap_via_ssh`.
    -- pem_cert = "/some/path/cert.pem",

    -- the path to an x509 PEM encoded CA chain file
    -- Omit this if you are using `bootstrap_via_ssh`.
    -- pem_ca = "/some/path/ca.pem",

    -- A set of paths to load additional CA certificates.
    -- Each entry can be either the path to a directory or to a PEM encoded
    -- CA file.  If an entry is a directory, then its contents will be
    -- loaded as CA certs and added to the trust store.
    -- Omit this if you are using `bootstrap_via_ssh`.
    -- pem_root_certs = { "/some/path/ca1.pem", "/some/path/ca2.pem" },

    -- explicitly control whether the client checks that the certificate
    -- presented by the server matches the hostname portion of
    -- `remote_address`.  The default is true.  This option is made
    -- available for troubleshooting purposes and should not be used outside
    -- of a controlled environment as it weakens the security of the TLS
    -- channel.
    -- accept_invalid_hostnames = false,

    -- the hostname string that we expect to match against the common name
    -- field in the certificate presented by the server.  This defaults to
    -- the hostname portion of the `remote_address` configuration and you
    -- should not normally need to override this value.
    -- expected_cn = "other.name",

    -- If true, connect to this domain automatically at startup
    -- connect_automatically = false,

    -- Specify an alternate read timeout
    -- read_timeout = 60,

    -- Specify an alternate write timeout
    -- write_timeout = 60,

    -- The path to the wezterm binary on the remote host
    -- remote_wezterm_path = "/home/myname/bin/wezterm"
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
config.tls_clients = {
  {
    name = 'server,name',
    bootstrap_via_ssh = 'server.hostname',
    remote_address = 'server.hostname:8080',
    local_echo_threshold_ms = 10,
  },
}
```

{{since('20221119-145034-49b9839f')}}

The lag indicator now defaults to disabled. It is recommended to display
the lag information in your status bar using [this
example](pane/get_metadata.md).

If you prefer to have the information overlaid on the content area, then
you can set `overlay_lag_indicator = true`, but note that I'd like to
remove that functionality in the future.
