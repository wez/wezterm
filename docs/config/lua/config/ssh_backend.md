---
tags:
  - ssh
---
# `ssh_backend = "Libssh"`

{{since('20211204-082213-a66c61ee9')}}

Sets which ssh backend should be used by default for the integrated ssh client.

Possible values are:

* `"Ssh2"` - use libssh2
* `"LibSsh"` - use libssh

Despite the naming, `libssh2` is not a newer version of `libssh`, they are
completely separate ssh implementations.

In prior releases, `"Ssh2"` was the only option.  `"LibSsh"` is the default
as it has broader support for newer keys and cryptography, and has clearer
feedback about authentication events that require entering a passphrase.

