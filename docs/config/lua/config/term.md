# `term = "xterm-256color"`

What to set the `TERM` environment variable to.  The default is
`xterm-256color`, which should provide a good level of feature
support without requiring the installation of additional terminfo
data.

If you want to get the most application support out of wezterm, then you may
wish to install a copy of the `wezterm` TERM definition:

```
tempfile=$(mktemp) \
  && curl -o $tempfile https://raw.githubusercontent.com/wez/wezterm/main/termwiz/data/wezterm.terminfo \
  && tic -x -o ~/.terminfo $tempfile \
  && rm $tempfile
```

You can then set `term = "wezterm"` in your `.wezterm.lua` config file.

Doing this will inform some software of newer, more advanced features such
as colored underlines, styled underlines (eg: undercurl).  If the system
you are using has a relatively outdated ncurses installation, the `wezterm`
terminfo will also enable italics and true color support.

If you are using WSL, wezterm will automatically populate `WSLENV` to properly set TERM, COLORTERM, TERM_PROGRAM and TERM_PROGRAM_VERSION.
See [this Microsoft blog post](https://devblogs.microsoft.com/commandline/share-environment-vars-between-wsl-and-windows/#what-are-environment-variables) for more information on how `WSLENV` works.

If your package manager installed the TERM definition in a non-standard location which will likely be the case if your are using nixpkgs/home-manager/NixOS then you need to set `TERMINFO_DIRS` before your profile is loaded.
The following snippet works if you installed `wezterm.terminfo` with nix into your user profile. Update the path to `TERMINFO_DIRS` to your location.

```lua
  set_environment_variables={
    TERMINFO_DIRS='/home/user/.nix-profile/share/terminfo',
    WSLENV='TERMINFO_DIRS'
  },
  term = "wezterm",
```
