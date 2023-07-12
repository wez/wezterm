---
tags:
  - multiplexing
---
# `default_mux_server_domain = "local"`

{{since('20230712-072601-f4abf8fd')}}

!!! note
    This option only applies to the standalone mux server.  For the equivalent option in
    the GUI, see [default_domain](default_domain.md)

When starting the mux server, by default wezterm will set the built-in
`"local"` domain as the default multiplexing domain.

The `"local"` domain represents processes that are spawned directly on the
local system.

This option allows you to change the default domain to some other domain, such
as an [ExecDomain](../ExecDomain.md).

It is *not* possible to configure a client multiplexing domain such as a TLS,
SSH or Unix domain as the default for the multiplexer server. That is
prohibited in order to prevent recursion when a client connects to the server.

