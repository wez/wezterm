# `wezterm.json_parse(string)`

*Since: nightly builds only*

Parses the supplied string as json and returns the equivalent lua values:

```
> wezterm.json_parse('{"foo":"bar"}')
{
    "foo": "bar",
}
```
