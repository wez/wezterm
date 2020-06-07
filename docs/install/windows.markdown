## Installing on Windows

64-bit Windows 10.0.17763 or later is required to run WezTerm.  You can download
a setup.exe style installer to guide the installation (requires admin privileges)
or a simple zip file and manage the files for yourself (no special privileges
required).

<a href="{{ windows_exe_stable }}" class="btn">Windows (setup.exe)</a>
<a href="{{ windows_exe_nightly }}" class="btn">Nightly Windows (setup.exe)</a>

WezTerm is available in a setup.exe style installer; the installer is produced
with Inno Setup and will install wezterm to your program files directory and
register that location in your PATH environment.  The installer can be run
as a GUI to guide you through the install, but also offers the [standard
Inno Setup command line options](https://jrsoftware.org/ishelp/index.php?topic=setupcmdline)
to configure/script the installation process.

<a href="{{ windows_zip_stable }}" class="btn">Windows (zip)</a>
<a href="{{ windows_zip_nightly }}" class="btn">Nightly Windows (zip)</a>

WezTerm is also available in a simple zip file that can be extracted and
run from anywhere, including a flash drive for a portable/relocatable
installation.

1. Download <a href="{{ windows_zip_stable }}">Release</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](../config/files.html)

### For `Scoop` users

If you prefer to use the command line to manage installing software,
then you may wish to try [Scoop](https://scoop.sh/).

Wezterm is available from the "Extras" bucket and once you have installed
scoop itself can be installed like so:

```bash
scoop bucket add extras
scoop install wezterm
```

