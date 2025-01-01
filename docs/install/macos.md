## Installing on macOS

The CI system builds the package on macOS Big Sur and should run on systems as
"old" as Mojave.  It may run on earlier versions of macOS, but that has not
been tested.

Starting with version 20210203-095643-70a364eb, WezTerm is a Universal binary
with support for both Apple Silicon and Intel hardware.

[:simple-apple: Download for macOS :material-tray-arrow-down:]({{ macos_zip_stable }}){ .md-button }
[:simple-apple: Nightly for macOS :material-tray-arrow-down:]({{ macos_zip_nightly }}){ .md-button }

1. Download <a href="{{ macos_zip_stable }}">Release</a>.
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder.
3. First time around, you may need to right click and select `Open` to allow launching
   the application that you've just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI.
4. To use wezterm binary from a terminal emulator, like `wezterm ls-fonts` you'll need to add the location to the wezterm binary folder that exists _inside_ the WezTerm.app, to your environment's $PATH value. For example, to add it to your `~/.zshrc` file, and assuming your WezTerm.app was installed to `/Applications`, add:
```sh
PATH="$PATH:/Applications/WezTerm.app/Contents/MacOS"
export PATH
```
5. Configuration instructions can be [found here](../config/files.md)

## Homebrew

WezTerm is available for [brew](https://brew.sh/) users:

```console
$ brew install --cask wezterm
```

If you'd like to use a nightly build:

```console
$ brew install --cask wezterm@nightly
```

!!! note
    For users who have previously used the cask named `wezterm-nightly`,
    homebrew has started issuing warnings: `Warning: Cask
    homebrew/cask-versions/wezterm-nightly was renamed to wezterm@nightly`. We
    recommend that you use `brew uninstall wezterm-nightly` to uninstall the
    previously installed version, and then reinstall the new version using the
    command above.

to upgrade to a newer nightly (normal `brew upgrade` will not upgrade it!):

```console
$ brew upgrade --cask wezterm@nightly --no-quarantine --greedy-latest
```

!!! note
    The `--greedy-latest` option in Homebrew forces the latest version of a
    formula to be installed, even if a version satisfying the formula's
    requirements is already installed. This can be useful when you want to
    ensure you have the most up-to-date version of a package, regardless of
    whether an older version meets the current dependency requirements.

## MacPorts

WezTerm is also available via [MacPorts](https://ports.macports.org/port/wezterm/summary):

```console
$ sudo port selfupdate
$ sudo port install wezterm
```

