# `color:srgba_u8()`

{{since('20220807-113146-c2fee766')}}

Returns a tuple of the internal SRGBA colors expressed
as unsigned 8-bit integers in the range 0-255:

```
> r, g, b, a = wezterm.color.parse("purple"):srgba_u8()
> print(r, g, b, a)
07:30:20.045 INFO logging > lua: 128 0 128 255
```
