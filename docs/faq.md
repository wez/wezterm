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

### Pasting or entering unicode in zsh looks broken

By default, zsh's line editor doesn't support combining character sequences.
Make sure that you have LANG and local configured correctly as shown above,
and then tell zsh to enable combining characters:

```
setopt COMBINING_CHARS
```

You'll want to put that into your zshrc so that it is always enabled.

See [this stackexchange
question](https://unix.stackexchange.com/questions/598440/zsh-indic-fonts-support-rendering-issue-which-is-working-fine-on-bash)
for more information.

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
local wezterm = require 'wezterm'

return {
  font = wezterm.font_with_fallback {
    'My Preferred Font',
    -- This font has a broader selection of Chinese glyphs than my preferred font
    'DengXian',
  },
}
```

See also [Troubleshooting Fonts](config/fonts.md#troubleshooting-fonts).

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

### Multiple characters being rendered/combined as one character?

`wezterm` supports [advanced font shaping](config/font-shaping.md), which,
amongst other things, allows for multiple characters/glyphs to be combined into
one [ligature](https://en.wikipedia.org/wiki/Ligature_(writing)). You may be
experiencing this if, e.g., `!=` becomes rendered as `≠` in `wezterm`.

If you are seeing this kind of "font combining" and wish to disable it, then
this is documented in [advanced font shaping options](config/font-shaping.md)
page.

## How to troubleshoot keys that don't work or produce weird characters!?

There are a number of layers in input processing that can influence this.

The first thing to note is that `wezterm` will always and only output `UTF-8`
encoded text.  Your `LANG` and locale related environment must be set to
reflect this; there is more information on that above.

If the key in question is produced in combination with Alt/Option then [this
section of the docs describes how wezterm processes
Alt/Option](config/keys.md), as well as options that influence that behavior.

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
issue](https://github.com/wezterm/wezterm/issues) and include the `xxd` hexdump,
and output from `env` and any other pertinent information about what you're
trying and why it doesn't match your expectations.

## I have `set convert-meta on` in my `~/.inputrc` and latin characters are broken!?

That setting causes Readline to re-encode latin-1 and other characters
as a different sequence (eg: `£` will have the high bit stripped and turn
it into `#`).

You should consider disabling that setting when working with a UTF-8
environment.

## How do I enable undercurl (curly underlines)?

Starting in version 20210314-114017-04b7cedd, WezTerm has support for colored
and curly underlines.

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
  && curl -o $tempfile https://raw.githubusercontent.com/wezterm/wezterm/master/termwiz/data/wezterm.terminfo \
  && tic -x -o ~/.terminfo $tempfile \
  && rm $tempfile
```

With that in place, you can then start neovim like this, and it should enable
undercurl:

```bash
env TERM=wezterm nvim
```

Note: on Windows, the ConPTY layer strips out the curly underline escape
sequences.  If you're missing this feature in your WSL instance, you will need
to use either `wezterm ssh` or
[multiplexing](multiplexing.md#connecting-into-windows-subsystem-for-linux)
to bypass ConPTY.

## I use Powershell for my shell, and I have problems with cursor keys in other apps

Powershell has [an open issue](https://github.com/PowerShell/PowerShell/issues/12268) where it
enables the [DECCKM](https://vt100.net/docs/vt510-rm/DECCKM) mode of the terminal and does
not restore it prior to launching external commands.

The consequence of enabling DECCKM is that cursor keys switch from being
reported as eg: `ESC [ A` (for UpArrow) to `ESC O A`.

Some applications don't know how to deal with this and as a consequence, won't
see the cursor keys.

This is not an issue in WezTerm; the same issue manifests in any terminal
emulator that runs powershell.

## I use X11 or Wayland and my mouse cursor theme doesn't seem to work

**What is this old school X11 mouse pointer thing?!**

Resolving the mouse cursor style in these environments is surprisingly complicated:

* Determine the XCursor theme:
  1. is `xcursor_theme` set in the wezterm configuration?
  2. X11: Does the root window publish the `XCursor.theme` resource? (You can manually run `xprop -root | grep RESOURCE_MANAGER | perl -pe 's/\\n/\n/g'  | grep -i cursor` to check for yourself)
  3. Wayland: from the `XCURSOR_THEME` environment variable
  4. Otherwise, assume `default`

* Determine the icon path:
  1. Is `XCURSOR_PATH` set in the environment? If so, use that.
  2. Construct a default path derived from some hard coded locations and the contents of the `XDG_DATA_HOME` and `XDG_DATA_DIRS` environment variables.

When a cursor is needed, the XCursor theme is tried first:

1. X11: the X Server must support the `RENDER` extension, version 0.5 or later, and support ARGB32
2. A set of candidate cursor names is produced for the desired cursor
3. For each location in the icon path, the XCursor theme and the candidate name are combined to produce a candidate file name
4. If the file exists, then wezterm will try to load it

If no XCursor was found, wezterm will fall back to using the default X11 cursor
font provided by the system.

{{since('20220624-141144-bd1b7c5d')}}

When troubleshooting xcursor issues, you can enable tracing by turning on the log level shown
below, and then moving the mouse over the wezterm window:

```
; WEZTERM_LOG=window::os::x11::cursor=trace wezterm
07:34:40.001  TRACE  window::os::x11::cursor > Constructing default icon path because $XCURSOR_PATH is not set
07:34:40.001  TRACE  window::os::x11::cursor > Using ~/.local/share because $XDG_DATA_HOME is not set
07:34:40.001  TRACE  window::os::x11::cursor > Using $XDG_DATA_DIRS location "/home/wez/.local/share/flatpak/exports/share:/var/lib/flatpak/exports/share:/usr/local/share/:/usr/share/"
07:34:40.001  TRACE  window::os::x11::cursor > icon_path is ["/home/wez/.local/share/icons", "/home/wez/.icons", "/home/wez/.local/share/flatpak/exports/share/icons", "/var/lib/flatpak/exports/share/icons", "/usr/local/share/icons", "/usr/share/icons", "/usr/share/pixmaps", "/home/wez/.cursors", "/usr/share/cursors/xorg-x11", "/usr/X11R6/lib/X11/icons"]
07:34:41.838  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/home/wez/.local/share/icons/Adwaita/cursors/xterm"
07:34:41.838  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/home/wez/.icons/Adwaita/cursors/xterm"
07:34:41.839  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/home/wez/.local/share/flatpak/exports/share/icons/Adwaita/cursors/xterm"
07:34:41.839  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/var/lib/flatpak/exports/share/icons/Adwaita/cursors/xterm"
07:34:41.839  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/usr/local/share/icons/Adwaita/cursors/xterm"
07:34:41.839  TRACE  window::os::x11::cursor > candidate for Some(Text) is "/usr/share/icons/Adwaita/cursors/xterm"
07:34:41.839  TRACE  window::os::x11::cursor > Some(Text) resolved to "/usr/share/icons/Adwaita/cursors/xterm"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.local/share/icons/Adwaita/cursors/top_left_arrow"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.local/share/icons/Adwaita/cursors/left_ptr"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.icons/Adwaita/cursors/top_left_arrow"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.icons/Adwaita/cursors/left_ptr"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.local/share/flatpak/exports/share/icons/Adwaita/cursors/top_left_arrow"
07:34:42.915  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/home/wez/.local/share/flatpak/exports/share/icons/Adwaita/cursors/left_ptr"
07:34:42.916  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/var/lib/flatpak/exports/share/icons/Adwaita/cursors/top_left_arrow"
07:34:42.916  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/var/lib/flatpak/exports/share/icons/Adwaita/cursors/left_ptr"
07:34:42.916  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/usr/local/share/icons/Adwaita/cursors/top_left_arrow"
07:34:42.916  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/usr/local/share/icons/Adwaita/cursors/left_ptr"
07:34:42.916  TRACE  window::os::x11::cursor > candidate for Some(Arrow) is "/usr/share/icons/Adwaita/cursors/top_left_arrow"
07:34:42.917  TRACE  window::os::x11::cursor > Some(Arrow) resolved to "/usr/share/icons/Adwaita/cursors/top_left_arrow"
```

## I'm on macOS and wezterm cannot find things in my PATH

On macOS, wezterm is typically launched directly by the Finder process and inherits
the default and fairly sparse macOS PATH environment.  That's sufficient for launching
your shell, which is then responsible for processing your rcfiles and setting up your
PATH.

However, if you want wezterm to directly spawn some other utility that isn't in that
basic PATH, wezterm will report that it cannot find it.

Probably the easiest to maintain solution is to change something like:

```lua
wezterm.action.SpawnCommandInNewWindow {
  args = { 'nvim', wezterm.config_file },
}
```

so that it explicitly spawns the command using your shell:

```lua
wezterm.action.SpawnCommandInNewWindow {
  args = {
    os.getenv 'SHELL',
    '-c',
    'nvim ' .. wezterm.shell_quote_arg(wezterm.config_file),
  },
}
```

another option is to explicitly use the full path to the program on your system,
something like:

```lua
wezterm.action.SpawnCommandInNewWindow {
  args = {
    wezterm.home_dir .. '/.local/bob/nvim-bin/nvim',
    wezterm.config_file,
  },
}
```

and another other option is to explicitly set the PATH up:

```lua
config.set_environment_variables = {
  -- prepend the path to your utility and include the rest of the PATH
  PATH = wezterm.home_dir .. '/.local/bob/nvim-bin:' .. os.getenv 'PATH',
}
```

and yet another option is to configure launchd to use a more expansive
PATH for all processes in your user session using `launchctl config user path`
doing something like this:

```console
$ sudo launchctl config user path <my path setting>
```

!!! warning
    Take care with setting the user path using this technique, as if you change
    that path in a way that system-provided utilities are lower priority than
    alternative software that you have installed, you may unexpectedly change
    the overall system behavior.

See also:

 * [set_environment_variables](config/lua/config/set_environment_variables.md)
 * [SpawnCommand](config/lua/SpawnCommand.md)
 * [wezterm.config_file](config/lua/wezterm/config_file.md)
 * [wezterm.shell_quote_arg](config/lua/wezterm/shell_quote_arg.md)
 * [how to set the PATH for Finder-launched applications](https://apple.stackexchange.com/q/51677/166425)
 * [what does launchctl config user path do?](https://stackoverflow.com/q/51636338/149111)

