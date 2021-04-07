## Installing on macOS

The CI system builds the package on macOS Big Sur and should run on systems as
"old" as Mojave.  It may run on earlier versions of macOS, but that has not
been tested.

Starting with version 20210203-095643-70a364eb, WezTerm is a Universal binary
with support for both Apple Silicon and Intel hardware.

<a href="{{ macos_zip_stable }}" class="btn">Download for macOS</a>
<a href="{{ macos_zip_nightly }}" class="btn">Nightly for macOS</a>
1. Download <a href="{{ macos_zip_stable }}">Release</a>
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder
3. First time around, you may need to right click and select `Open` to allow launching
   the application that your just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI
4. Configuration instructions can be [found here](../config/files.html)

## Homebrew

WezTerm is available for [brew](https://brew.sh/) users in a tap:

```bash
$ brew tap wez/wezterm
$ brew install wezterm
```

If you'd like to use a nightly build you can perform a head install:

```bash
$ brew install --HEAD wezterm
```

to upgrade to a newer nightly, it is simplest to remove then
install:

```bash
$ brew rm wezterm
$ brew install --HEAD wezterm
```

## MacPorts

WezTerm is also available via [MacPorts](https://ports.macports.org/port/wezterm/summary):

```bash
$ sudo port selfupdate
$ sudo port install wezterm
```
