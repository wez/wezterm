# `wezterm.mux.get_domain(name_or_id)`

*Since: nightly builds only*

Resolves `name_or_id` to a domain and returns a
[MuxDomain](../MuxDomain/index.md) object representation of it.

`name_or_id` can be:

* A domain name string to resolve the domain by name
* A domain id to resolve the domain by id
* `nil` or omitted to return the current default domain
* other lua types will generate a lua error

If the name or id don't map to a valid domain, this function will return `nil`.

