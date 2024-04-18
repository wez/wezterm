# `wezterm.serde.toml_decode(string)`

{{since('nightly')}}

Parses the supplied string as `toml` and returns the equivalent `lua` values:

```
> wezterm.serde.toml_decode('foo = "bar"')
{
    "foo": "bar",
}
```
