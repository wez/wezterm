---
tags:
  - multiplexing
  - ssh
---
# `mux_enable_ssh_agent = true`

{{since('nightly')}}

When set to `true` (the default), wezterm will configure the `SSH_AUTH_SOCK`
environment variable for panes spawned in the `local` domain.

The auth sock will point to a symbolic link that will in turn be pointed to the
authentication socket associated with the most recently active multiplexer
client.

You can review the authentication socket that will be used for various clients
by running `wezterm cli list-clients` and inspecting the `SSH_AUTH_SOCK`
column.

The symlink is updated within (at the time of writing this documentation) 100ms
of the active Mux client changing.

You can set `mux_enable_ssh_agent = false` to prevent wezterm from assigning
`SSH_AUTH_SOCK` or updating the symlink.

