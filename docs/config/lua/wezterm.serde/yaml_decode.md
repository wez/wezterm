# `wezterm.serde.yaml_decode(string)`

{{since('nightly')}}

Parses the supplied string as `yaml` and returns the equivalent `lua` values:

```
> wezterm.serde.yaml_decode('---\n# comment\nfoo: "bar"')
{
    "foo": "bar",
}
```
