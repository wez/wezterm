# `wezterm.mux.set_default_domain(MuxDomain)`

{{since('20230320-124340-559cb7b0')}}

Assign a new default domain in the mux.

The domain that you assign here will override any configured
[default_domain](../config/default_domain.md) or the implicit assignment of the
default domain that may have happened as a result of starting wezterm via
`wezterm connect` or `wezterm serial`.
