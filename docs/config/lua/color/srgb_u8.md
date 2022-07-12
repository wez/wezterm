## `color:srgba_u8()`

*Since: nightly builds only*

Returns a tuple of the internal SRGBA colors expressed
as unsigned 8-bit integers in the range 0-255:

```lua
> r, g, b, a = wezterm.color.parse("purple"):srgba_u8()
> print(r, g, b, a)
07:30:20.045 INFO logging > lua: 128 0 128 255
```
