# Frequently Asked Questions

## Some glyphs look messed up, why is that?

There's a surprisingly amount of work that goes into rendering text,
and if you're connected to a remote host, it may span both systems.

### LANG and locale

Terminals operate on byte streams and don't necessarily know anything about the
encoding of the text that you're sending through.  The unix model for this is
that the end user (that's you!) will instruct the applications that you're
running to use a particular locale to interpret the byte stream.

It is common for these environment variables to not be set, or to be set to
invalid values by default!

You need to select a unicode locale for best results; for example:

```
export LANG=en_US.UTF-8
# You don't strictly need this collation, but most technical people
# probably want C collation for sane results
export LC_COLLATE=C
```

If you have other `LC_XXX` values in your environment, either remove
them from your environment (if applicable) or adjust them to use a
UTF-8 locale.

You can run `locale -a` to list the available locales on your system.

You need to make sure that this setting applies both locally and on systems
that you log in to via ssh or the mux connection protocol.

If you're seeing multiple garbage characters in your terminal in place of
what should be a single glyph then you most likely have a problem with your
locale environment variables.

### Fonts and fallback

If you have configured the use of a font that contains only latin characters
and then try to display a glyph that isn't present in that font (perhaps an
emoji, or perhaps some kanji) then wezterm will try to locate a fallback
font that does contain that glyph.

Wezterm uses freetype and harfbuzz to perform font shaping and rendering in a
cross platform way, and as a consequence, doesn't have access to the system
font fallback selection.  Instead it has a short list of fallback fonts that
are likely to be present on the system and tries to use those.

If you're seeing the unicode replacement character, a question mark or in
the worst cases spaces where a glyph should be, then you have an issue with
font fallback.

You can resolve this by explicitly adding fallback font(s) the have the glyphs
that you need in your `wezterm.toml`:

```toml
[[font.font]]
family = "My Preferred Font"

# This font has a broader selection of Chinese glyphs than my preferred font
[[font.font]]
family = "DengXian"
```

### Some (but not all) Emoji don't render properly

To some extent this issue can manifest in a similar way to the LANG and locale
issue.  There are different versions of the Emoji specifications and the level
of support in different applications can vary.  Emoji can be comprised from a
sequence of codepoints and some combine in interesting ways such as a foot and
a skin tone.  Applications that don't support this correctly may end up
emitting incorrect output.  For example, pasting some emoji into the zsh REPL
confuses its input parser and results in broken emoji output.  However, if you
were to emit that same emoji from a script, wezterm would render it correctly.

If you're seeing this sort of issue, then you may be able to upgrade the
affected application on that system to see if a newer version resolves that
issue.


