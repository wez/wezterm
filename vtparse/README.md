# vtparse

This is an implementation of a parser for escape and control sequences.
It is based on the [DEC ANSI Parser](https://vt100.net/emu/dec_ansi_parser).

It has been modified slightly to support UTF-8 sequences.

`vtparse` is the lowest level parser; it categorizes the basic
types of sequences but does not ascribe any semantic meaning
to them.

You may wish to look at `termwiz::escape::parser::Parser` in the
[termwiz](https://docs.rs/termwiz) crate if you're looking for semantic
parsing.

## Comparison with the `vte` crate

`vtparse` has support for dynamically sized OSC buffers, which makes
it suitable for processing large escape sequences, such as those
used by the `iTerm2` image protocol.
