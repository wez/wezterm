# `key_map_preference = "Mapped"`

*Since: nightly builds only*

Controls how keys without an explicit `phys:` or `mapped:` prefix are treated.

If `key_map_preference = "Mapped"` (the default), then `mapped:` is assumed. If
`key_map_preference = "Physical"` then `phys:` is assumed.

Default key assignments also respect `key_map_preference`.

