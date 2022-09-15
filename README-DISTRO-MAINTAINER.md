# Notes for distro maintainers

`ci/deploy.sh` is a script that is used to build packages in wezterm's CI.
It is likely a bit more coarse than most distros would want in their
official packages, but it should give a sense of what is intended to go where.

## Versioning

WezTerms version number is derived from the date of the commit from which it
was released:

```
git -c "core.abbrev=8" show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S"
```

If you are not building wezterm from its git repo, wezterm will read a file named
`.tag` in the root of its source tree to determine the version.

Please package using the `wezterm-YYYYMMDD-HHMMSS-HASH-src.tar.gz` release
asset from the release, rather than the automatic GitHub source tarball.
Not only is it a smaller download, but it already contains an appropriate
`.tag` file with the release version baked into it.

If you must decorate wezterm's version, then it is recommend that you append
your supplemental extra version information on the end of the wezterm's
version.

For example: `YYYYMMDD-HHMMSS-HASH-EXTRA`.

If `-` is illegal in your package management system, then it is recommended
that you substitute either `.` or `_` to separate the portions of the version
string.

## Binaries

* `wezterm-mux-server` - multiplexer server. No gui required. Likely want this
  in a separate package from `wezterm-gui` so it can run on a headless system.
* `wezterm-gui` - the GUI portion of the terminal
* `wezterm` - the CLI and frontend that knows how to launch the GUI. It is
  desirable to have this available to both the multiplexer server and the gui.
* `strip-ansi-escapes` - a utility that can filter escapes out of stdin; useful
  for de-fanging text when composing eg: OSC 0/1 title text.

## Additional Resources

* `assets/shell-integration`, `assets/shell-completion`: should be deployed along with the `wezterm` executable
* `assets/wezterm.desktop`, `assets/wezterm.appdata.xml`, `assets/wezterm-nautilus.py`: should be deployed along with `wezterm-gui`

## Building wezterm

It is recommended that you enable the `distro-defaults` rust feature
when building for a distro (`cargo build --release -p wezterm-gui --features distro-defaults`).

It has the following effects:

* `check_for_updates` will default to `false`

### Un-bundling vendored fonts

By default, wezterm will compile in a handful of fonts in order to provide a
consistent out of the box experience on all platforms with minimal installation
hassle.

If your distribution offers those fonts as installable packages, then it is
recommended that you skip compiling in that font by disabling the associated
feature:

* `vendor-nerd-font-symbols-font` - causes [Symbols Nerd Font
  Mono](https://github.com/ryanoasis/nerd-fonts/blob/master/patched-fonts/NerdFontsSymbolsOnly/complete/Symbols-1000-em%20Nerd%20Font%20Complete%20Mono.ttf)
  to be compiled in.
* `vendor-jetbrains-font` - causes `JetBrains Mono` to be compiled in
* `vendor-roboto-font` - causes `Roboto` to be compiled in
* `vendor-noto-emoji-font` - causes `Noto Color Emoji` to be compiled in.
* `vendored-fonts` - causes all of the above `vendor-*-font` features to be enabled

Note that wezterm requires at least the following fonts to be available, either
on the system or built-in, in its default configuration, in order to start
correctly:

* `JetBrains Mono`
* `Roboto`

If there are other behaviors that you'd like to change from the default, please
raise issue(s) for them so that we can figure out how to make it easier for you
to maintain your wezterm package.

### Building without wayland support

If your distro doesn't include any support for Wayland, you will need to
disable that feature when you build wezterm:

```
cargo build --release -p wezterm-gui --no-default-features --features distro-defaults,vendored-fonts
```

