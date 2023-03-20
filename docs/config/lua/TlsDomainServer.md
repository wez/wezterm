# TlsDomainServer

The `TlsDomainServer` struct specifies information about how to define
the server side of a [TLS Domain](../../multiplexing.md#tls-domains).

It is a lua object with the following fields:

```lua
config.tls_servers = {
  {
    -- The address:port combination on which the server will listen
    -- for client connections
    bind_address = 'server.hostname:8080',

    -- the path to an x509 PEM encoded private key file.
    -- You can omit this if your tls_client is using bootstrap_via_ssh.
    -- pem_private_key = "/path/to/key.pem",

    -- the path to an x509 PEM encoded certificate file.
    -- You can omit this if your tls_client is using bootstrap_via_ssh.
    -- pem_cert = "/path/to/cert.pem",

    -- the path to an x509 PEM encoded CA chain file.
    -- You can omit this if your tls_client is using bootstrap_via_ssh.
    -- pem_ca = "/path/to/chain.pem",

    -- A set of paths to load additional CA certificates.
    -- Each entry can be either the path to a directory
    -- or to a PEM encoded CA file.  If an entry is a directory,
    -- then its contents will be loaded as CA certs and added
    -- to the trust store.
    -- You can omit this if your tls_client is using bootstrap_via_ssh.
    -- pem_root_certs = { "/some/path/ca1.pem", "/some/path/ca2.pem" },
  },
}
```
