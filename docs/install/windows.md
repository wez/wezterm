## Installing on Windows

64-bit Windows 10.0.17763 or later is required to run WezTerm; running on
earlier versions of Windows is not possible, as WezTerm requires [Pseudo
Console support that was first released in Windows
10.0.17763](https://devblogs.microsoft.com/commandline/windows-command-line-introducing-the-windows-pseudo-console-conpty/).

You can download a setup.exe style installer to guide the installation
(requires admin privileges) or a simple zip file and manage the files for
yourself (no special privileges required).

[:simple-windows: Windows (setup.exe) :material-tray-arrow-down:]({{ windows_exe_stable }}){ .md-button }
[:simple-windows: Nightly Windows (setup.exe) :material-tray-arrow-down:]({{ windows_exe_nightly }}){ .md-button }

WezTerm is available in a setup.exe style installer; the installer is produced
with Inno Setup and will install wezterm to your program files directory and
register that location in your PATH environment.  The installer can be run
as a GUI to guide you through the install, but also offers the [standard
Inno Setup command line options](https://jrsoftware.org/ishelp/index.php?topic=setupcmdline)
to configure/script the installation process.

[:simple-windows: Windows (zip) :material-tray-arrow-down:]({{ windows_zip_stable }}){ .md-button }
[:simple-windows: Nightly Windows (zip) :material-tray-arrow-down:]({{ windows_zip_nightly }}){ .md-button }

WezTerm is also available in a simple zip file that can be extracted and
run from anywhere, including a flash drive for a portable/relocatable
installation.

1. Download <a href="{{ windows_zip_stable }}">Release</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](../config/files.md)

### For `winget` users

If you prefer to use the command line to manage installing software,
then you may wish to try [winget](https://github.com/microsoft/winget-cli#installing-the-client).
`winget` is installed as part of the [App Installer](https://www.microsoft.com/en-us/p/app-installer/9nblggh4nns1)
that is available from the Microsoft Store.

Once you have `winget`, you can install wezterm like so:

```console
$ winget install wez.wezterm
```

and to later upgrade it:

```console
$ winget upgrade wez.wezterm
```

### For `Scoop` users

Another option if you prefer to use the command line to manage installing
software, is [Scoop](https://scoop.sh/).

Wezterm is available from the "Extras" bucket and once you have installed
scoop itself can be installed like so:

```console
$ scoop bucket add extras
$ scoop install wezterm
```

### For `Chocolatey` users

If you prefer to use [Chocolatey](https://chocolatey.org) to manage software,
wezterm is available from the Community Repository.  It can be installed like
so:

```console
$ choco install wezterm -y
```
