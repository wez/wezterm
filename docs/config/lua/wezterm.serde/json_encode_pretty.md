# `wezterm.serde.json_encode_pretty(value)`

{{since('nightly')}}

Encodes the supplied `lua` value as a pretty-printed string of `json`: 

```
> wezterm.serde.json_encode_pretty({foo = "bar"})
"{\n  \"foo\": \"bar\"\n}"
```
