# `wezterm.serde.json_encode(value)`

{{since('nightly')}}

Encodes the supplied `lua` value as `json`:

```
> wezterm.serde.json_encode({foo = "bar"})
"{\"foo\":\"bar\"}"
```
