---
tags:
  - unicode
---
# `cell_widths = {}`

{{since('nightly')}}

The character width recommended by the Unicode standard is occasionally
inconsistent and may not align with linguistic tradition.

- circled numbers width: ⓪①..⑳㉑
- lowercase Roman numerals width: ⅹⅺⅻ
- Nerd Font (Private Use Area) character width
- [ambiguous character width](http://www.unicode.org/reports/tr11/#Ambiguous) for CJK text
- square emojis defined as EAW=Neutral

The `cell_widths` configuration parameter allows users to override the
default character width. This setting takes priority over the
`treat_east_asian_ambiguous_width_as_wide` setting.


[Nerd Font](https://www.nerdfonts.com/) has square glyphs and is an
example of half advance width.  Below is a configuration example that
treats these character widths as full-width:

```lua
config.cell_widths = {
  { first = 0xe000, last = 0xf8ff, width = 2 },
  { first = 0xf0000, last = 0xf1fff, width = 2 },
}
```

Note that changing this setting may have consequences for layout in text UI
applications if their expectation of width differs from your choice of
configuration.

For example, Vim has a built-in function
[setcellwidths()](https://vimhelp.org/builtin.txt.html#setcellwidths%28%29),
and shells like Bash or Zsh determine character width based on the
glibc locale.

See also: [treat_east_asian_ambiguous_width_as_wide](treat_east_asian_ambiguous_width_as_wide.md)
