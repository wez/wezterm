# `Time:format_utc(format)`

*Since: nightly builds only*

Formats the time object as a string, using UTC date/time representation of the time.

The format string supports the [set of formatting placeholders described here](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).

```lua
> wezterm.time.now():format_utc("%Y-%m-%d %H:%M:%S")
"2022-07-17 18:14:15"
> wezterm.time.now():format("%Y-%m-%d %H:%M:%S")
"2022-07-17 11:14:15"
```

See also [Time:format()](format.md).

