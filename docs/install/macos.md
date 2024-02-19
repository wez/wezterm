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
$ brew tap homebrew/cask-versions
$ brew install --cask wez/wezterm/wezterm-nightly --no-quarantine
```

> 1. `--cask` needs to be set, as the cask name exists in homebrew already and will result in the following error:
> ```
> Error: Cask wezterm-nightly exists in multiple taps:
  homebrew/cask-versions/wezterm-nightly
  wez/wezterm/wezterm-nightly
  ```
> 2. `--no-quarantine` is set, otherwise you will receive the warning that Wezterm cannot be opened because the developer cannot be verified. This message is from [Gatekeeper](https://support.apple.com/en-mide/102445).
> 3. To remove the above, you can remove the quarantine flag by using (-r is recursive, -d is delete, .app is a directory on MacOS):
> ```sh
xattr -rd com.apple.FinderInfo /Applications/WezTerm.app
````

to upgrade to a newer nightly (normal `brew upgrade` will not upgrade it!):

```console
$ brew upgrade --cask wezterm-nightly --no-quarantine --greedy-latest
```

## MacPorts

WezTerm is also available via [MacPorts](https://ports.macports.org/port/wezterm/summary):

```console
$ sudo port selfupdate
$ sudo port install wezterm
```

