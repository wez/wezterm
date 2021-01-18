# Frequently Asked Questions

## Unicode glyphs render as underscores in my tmux!

This is likely an issue with LANG and locale.  `tmux` will substitute unicode
glyphs with underscores if it believes that your environment doesn't support
UTF-8.

If you're running on macOS, upgrade to `20200620-160318-e00b076c` or newer and
WezTerm will automatically set `LANG` appropriately.

Note that if you change your environment you will likely need to kill and
restart your tmux server before it will take effect.

You probably should also review [this relevant section from the
TMUX FAQ](https://github.com/tmux/tmux/wiki/FAQ#how-do-i-use-utf-8), and
read on for more information about LANG and locale below.

## Some glyphs look messed up, why is that?

There's a surprisingly amount of work that goes into rendering text,
and if you're connected to a remote host, it may span both systems.
Read on for some gory details!

### LANG and locale

Terminals operate on byte streams and don't necessarily know anything about the
encoding of the text that you're sending through.  The unix model for this is
that the end user (that's you!) will instruct the applications that you're
running to use a particular locale to interpret the byte stream.

It is common for these environment variables to not be set, or to be set to
invalid values by default!

If you're running on macOS, upgrade to `20200620-160318-e00b076c` or newer
and WezTerm will automatically set `LANG` appropriately.

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
that you need in your `.wezterm.lua`:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font_with_fallback({
    "My Preferred Font",
    -- This font has a broader selection of Chinese glyphs than my preferred font
    "DengXian"
  })
}
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

## How to troubleshoot keys that don't work or produce weird characters!?

There are a number of layers in input processing that can influence this.

The first thing to note is that `wezterm` will always and only output `UTF-8`
encoded text.  Your `LANG` and locale related environment must be set to
reflect this; there is more information on that above.

If the key in question is produced in combination with Alt/Option then [this
section of the docs describes how wezterm processes
Alt/Option](config/keys.html), as well as options that influence that behavior.

The next thing to verify is what byte sequences are being produced when you
press keys.  I generally suggest running `xxd`, pressing the relevant key, then
enter, then CTRL-D.  This should show a hex dump of the the byte sequence.
This step helps to isolate the input from input processing layers in other
applications.

Interactive Unix programs generally depend upon the `TERM` environment variable
being set appropriately.  `wezterm` sets this to `xterm-256color` by default,
because wezterm aims to be compatible with with the settings defined by that
terminfo entry.  Setting TERM to something else can change the byte sequences
that interactive applications expect to see for some keys, effectively
disabling those keys.

On top of this, a number of programs use libraries such as GNU readline
to perform input processing.  That means that settings in your `~/.inputrc`
may changing the behavior of `bash`.  Verify any settings in there that
might influence how input is resolved and see the question below
about `convert-meta`!

If you are using `tmux` be aware that it introduces its own set of input/output
processing layers that are also sensitive to `LANG`, `TERM` and locale and how
they are set in the environment of the tmux server when it was spawned, the
tmux client and inside the processes spawned by tmux.  It is generally best to
troubleshoot input/output weirdness independent of tmux first to minimize the
number of variables!

If after experimenting with your environment and related settings you believe
that wezterm isn't sending the correct input then please [open an
issue](https://github.com/wez/wezterm/issues) and include the `xxd` hexdump,
and output from `env` and any other pertinent information about what you're
trying and why it doesn't match your expectations.

## I have `set convert-meta on` in my `~/.inputrc` and latin characters are broken!?

That setting causes Readline to re-encode latin-1 and other characters
as a different sequence (eg: `£` will have the high bit stripped and turn
it into `#`).

You should consider disabling that setting when working with a UTF-8
environment.

## How do I enable undercurl (curly underlines)?

Starting in the nightly builds, WezTerm has support for colored and curly underlines.

The relevant escape sequences are:

```
 CSI 24 m   -> No underline
 CSI 4 m    -> Single underline
 CSI 4:0 m  -> No underline
 CSI 4:1 m  -> Single underline
 CSI 4:2 m  -> Double underline
 CSI 4:3 m  -> Curly underline
 CSI 4:4 m  -> Dotted underline
 CSI 4:5 m  -> Dashed underline

 CSI 58:2::R:G:B m   -> set underline color to specified true color RGB
 CSI 58:5:I m        -> set underline color to palette index I (0-255)
 CSI 59              -> restore underline color to default
```

You can try these out in your shell; this example will print the various
underline styles with a red underline:

```bash
$ printf "\x1b[58:2::255:0:0m\x1b[4:1msingle\x1b[4:2mdouble\x1b[4:3mcurly\x1b[4:4mdotted\x1b[4:5mdashed\x1b[0m\n"
```

To use this in vim, add something like the following to your `.vimrc`:

```vim
let &t_Cs = "\e[4:3m"
let &t_Ce = "\e[4:0m"
hi SpellBad   guisp=red gui=undercurl guifg=NONE guibg=NONE \
     ctermfg=NONE ctermbg=NONE term=underline cterm=undercurl ctermul=red
hi SpellCap   guisp=yellow gui=undercurl guifg=NONE guibg=NONE \
     ctermfg=NONE ctermbg=NONE term=underline cterm=undercurl ctermul=yellow
```

If you are a neovim user then you will need to install a terminfo file that
tells neovim about this support.

You may wish to try these steps to install a copy of a `wezterm` terminfo file;
this will compile a copy of the terminfo and install it into your `~/.terminfo`
directory:

```bash
tempfile=$(mktemp) \
  && curl -o $tempfile https://raw.githubusercontent.com/wez/wezterm/master/termwiz/data/wezterm.terminfo \
  && tic -x -o ~/.terminfo $tempfile \
  && rm $tempfile
```

With that in place, you can then start neovim like this, and it should enable
undercurl:

```bash
env TERM=wezterm nvim
```

