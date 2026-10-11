---
tags:
  - scroll_bar
---
# `erase_display_scrolls_into_scrollback = false`

{{since('nightly')}}

`CSI 2 J` (*erase in display, all*) erases the whole screen. The spec, and
xterm, erase it in place: the rows that were on screen are overwritten and
nothing is added to the scrollback. `clear -x`, and the `clear-screen` binding
that most shells put on `Ctrl-L`, send `CSI H CSI 2 J`, so by default the
screen you just cleared is gone rather than scrolled away. (Plain `clear` also
sends `CSI 3 J`, which erases the scrollback itself.)

When this option is set to `true`, wezterm first scrolls the screen contents
into the scrollback and then erases the screen, so scrolling up shows what was
there before the clear.

```lua
config.erase_display_scrolls_into_scrollback = true
```

Only the rows up to the last row that holds something are scrolled, so
clearing a mostly-empty screen doesn't push a screenful of blank rows into the
scrollback. The option has no effect on the [alternate
screen](../../../escape-sequences.md), which has no scrollback, and none on
`CSI 0 J`, `CSI 1 J` or `CSI 3 J`.

This is deliberately not the default: it is a departure from the specified
behavior of the sequence, and a full-screen application that repaints with
`CSI 2 J` instead of using the alternate screen will add a copy of each frame
to your scrollback.

If you would rather not change how the terminal interprets the sequence, you
can get the same effect from the shell side by having `Ctrl-L` scroll the
screen before clearing it; see
[#2405](https://github.com/wezterm/wezterm/issues/2405) for an example that
works in any terminal.
