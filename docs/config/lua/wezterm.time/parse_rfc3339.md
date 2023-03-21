# `wezterm.time.parse_rfc3339(str)`

{{since('20220807-113146-c2fee766')}}

Parses a string that is formatted according to [RFC
3339](https://datatracker.ietf.org/doc/html/rfc3339) and returns a
[Time](Time/index.md) object representing that time.

Will raise an error if the input string cannot be parsed according to RFC 3339.

