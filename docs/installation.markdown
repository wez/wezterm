
## Installing on Windows

Windows 10 or later is required to run WezTerm.

<a href="{{ windows_zip_stable }}" class="btn">Download for Windows</a>
<a href="{{ windows_zip_nightly }}" class="btn">Nightly for Windows</a>
1. Download <a href="{{ windows_zip_stable }}">Release</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](config/index.html)

## Installing on macOS

The CI system builds the package on macOS Mojave (10.14).  It may run on earlier
versions of macOS, but that has not been tested.

<a href="{{ macos_zip_stable }}" class="btn">Download for macOS</a>
<a href="{{ macos_zip_nightly }}" class="btn">Nightly for macOS</a>
1. Download <a href="{{ macos_zip_stable }}">Release</a>
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder
3. First time around, you may need to right click and select `Open` to allow launching
   the application that your just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI
4. Configuration instructions can be [found here](config/index.html)

## Installing on Ubuntu and Debian-based Systems

The CI system builds `.deb` files for a variety of Ubuntu and Debian distributions.
These are often compatible with other Debian style systems; if you don't find one
that exactly matches your system you can try installing one from an older version
of your distribution, or use one of the Debian packages linked below.  Failing that,
you can try the AppImage download which should work on most Linux systems.

|Distro      | Stable           | Nightly             |
|------------|------------------|---------------------|
|Ubuntu16    |[{{ ubuntu16_deb_stable_asset }}]({{ ubuntu16_deb_stable }}) |[{{ ubuntu16_deb_nightly_asset }}]({{ ubuntu16_deb_nightly }})|
|Ubuntu18    |[{{ ubuntu18_deb_stable_asset }}]({{ ubuntu18_deb_stable }}) |[{{ ubuntu18_deb_nightly_asset }}]({{ ubuntu18_deb_nightly }})|
|Ubuntu19    |[{{ ubuntu19_deb_stable_asset }}]({{ ubuntu19_deb_stable }}) |[{{ ubuntu19_deb_nightly_asset }}]({{ ubuntu19_deb_nightly }})|
|Ubuntu20    | (not yet) |[{{ ubuntu20_deb_nightly_asset }}]({{ ubuntu20_deb_nightly }})|
|Debian9     |[{{ debian9_deb_stable_asset }}]({{ debian9_deb_stable }}) |[{{ debian9_deb_nightly_asset }}]({{ debian9_deb_nightly }})|
|Debian10    |[{{ debian10_deb_stable_asset }}]({{ debian10_deb_stable }}) |[{{ debian10_deb_nightly_asset }}]({{ debian10_deb_nightly }})|

To download and install from the CLI, you can use something like this, which
shows how to install the Ubuntu 16 package:

```bash
curl -LO {{ ubuntu16_deb_stable }}
sudo apt install -y ./{{ ubuntu16_deb_stable_asset }}
```

* The package installs `/usr/bin/wezterm` and `/usr/share/applications/org.wezfurlong.wezterm.desktop`
* Configuration instructions can be [found here](config/index.html)

## Installing on Fedora and rpm-based Systems

The CI system builds `.rpm` files on CentOS and Fedora systems.
These are likely compatible with other rpm-based distributions.
Alternatively, you can try the AppImage download with should work
on most Linux systems.

|Distro      | Stable           | Nightly             |
|------------|------------------|---------------------|
|CentOS7     |[{{ centos7_rpm_stable_asset }}]({{ centos7_rpm_stable }}) |[{{ centos7_rpm_nightly_asset }}]({{ centos7_rpm_nightly }})|
|CentOS8     |[{{ centos8_rpm_stable_asset }}]({{ centos8_rpm_stable }}) |[{{ centos8_rpm_nightly_asset }}]({{ centos8_rpm_nightly }})|
|Fedora31    |[{{ fedora31_rpm_stable_asset }}]({{ fedora31_rpm_stable }}) |[{{ fedora31_rpm_nightly_asset }}]({{ fedora31_rpm_nightly }})|
|Fedora32    |[{{ fedora32_rpm_stable_asset }}]({{ fedora32_rpm_stable }}) |[{{ fedora32_rpm_nightly_asset }}]({{ fedora32_rpm_nightly }})|

To download and install form the CLI you can use something like this, which
shows how to install the Fedora 31 package:

```bash
sudo dnf install -y {{ fedora31_rpm_stable }}
```

* The package installs `/usr/bin/wezterm` and `/usr/share/applications/org.wezfurlong.wezterm.desktop`
* Configuration instructions can be [found here](config/index.html)

## Installing on Linux via AppImage

If you have some other Linux system, or otherwise prefer AppImage over your
system package format, you can download a build by following these steps.

<a href="{{ ubuntu16_AppImage_stable }}" class="btn">AppImage</a>
<a href="{{ ubuntu16_AppImage_nightly }}" class="btn">Nightly AppImage</a>

```bash
curl -LO {{ ubuntu16_AppImage_stable }}
chmod +x {{ ubuntu16_AppImage_stable_asset }}
```

You may then execute the appimage directly to launch wezterm, with no
specific installation steps required:

```bash
./{{ ubuntu16_AppImage_stable_asset }}
```

That said, you may wish to make it a bit more convenient:

```bash
mkdir ~/bin
mv ./{{ ubuntu16_AppImage_stable_asset }} ~/bin/wezterm
~/bin/wezterm
```

* Configuration instructions can be [found here](config/index.html)

## Raw Linux Binary

Another option for linux is a raw binary archive.  These are the same binaries that
are built for Ubuntu but provided in a tarball.

<a href="{{ linux_raw_bin_stable }}" class="btn">Download raw Linux binaries</a>
<a href="{{ linux_raw_bin_nightly }}" class="btn">Nightly raw Linux binaries</a>

## Installing from source

If your system isn't covered by the list above, then you can build it for yourself.
WezTerm should run on any modern unix as well as Windows 10 and macOS.

* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html)
* Rust version 1.41 or later is required
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](https://github.com/wez/wezterm/blob/master/get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](https://github.com/wez/wezterm/blob/master/CONTRIBUTING.md)

If you don't plan to submit a pull request to the wezterm repo, you can
download a smaller source tarball using these steps:

```bash
curl https://sh.rustup.rs -sSf | sh -s
curl -LO {{ src_stable }}
tar -xzf {{ src_stable_asset }}
cd {{ src_stable_dir }}
sudo ./get-deps
cargo build --release
cargo run --release -- start
```

Alternatively, use the full git repo:

```bash
curl https://sh.rustup.rs -sSf | sh -s
git clone --depth=1 --branch=master --recursive https://github.com/wez/wezterm.git
cd wezterm
git submodule update --init --recursive
sudo ./get-deps
cargo build --release
cargo run --release -- start
```
