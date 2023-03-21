# `wezterm.time.parse(str, format)`

{{since('20220807-113146-c2fee766')}}

Parses a string that is formatted according to the supplied format string:

```
> wezterm.time.parse("1983 Apr 13 12:09:14.274 +0000", "%Y %b %d %H:%M:%S%.3f %z")
"Time(utc: 1983-04-13T12:09:14.274+00:00)"
```

The format string supports the [set of formatting placeholders described here](https://docs.rs/chrono/latest/chrono/format/strftime/index.html).
