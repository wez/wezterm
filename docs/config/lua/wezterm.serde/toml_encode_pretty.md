# `wezterm.serde.toml_encode_pretty(value)`

{{since('nightly')}}

Encodes the supplied `lua` value as a pretty-printed string of `toml`: 

```
> wezterm.serde.toml_encode_pretty({foo = { "bar", "baz", "qux" } })
"foo = [\n    \"bar\",\n    \"baz\",\n    \"qux\",\n]\n"
```
