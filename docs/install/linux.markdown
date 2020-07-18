## Installing on Linux using AppImage

WezTerm is available in [AppImage](https://appimage.org/) format; a
self-contained single file that doesn't require installation or
any special privileges to run, and that is compatible with a wide
range of Linux distributions.

Download and make the file executable and you're ready to run!

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

* Configuration instructions can be [found here](../config/files.html)

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
|Ubuntu20    |[{{ ubuntu20_deb_stable_asset }}]({{ ubuntu20_deb_stable }})  |[{{ ubuntu20_deb_nightly_asset }}]({{ ubuntu20_deb_nightly }})|
|Debian9     |[{{ debian9_deb_stable_asset }}]({{ debian9_deb_stable }}) |[{{ debian9_deb_nightly_asset }}]({{ debian9_deb_nightly }})|
|Debian10    |[{{ debian10_deb_stable_asset }}]({{ debian10_deb_stable }}) |[{{ debian10_deb_nightly_asset }}]({{ debian10_deb_nightly }})|

To download and install from the CLI, you can use something like this, which
shows how to install the Ubuntu 16 package:

```bash
curl -LO {{ ubuntu16_deb_stable }}
sudo apt install -y ./{{ ubuntu16_deb_stable_asset }}
```

* The package installs `/usr/bin/wezterm` and `/usr/share/applications/org.wezfurlong.wezterm.desktop`
* Configuration instructions can be [found here](../config/files.html)

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
* Configuration instructions can be [found here](../config/files.html)

## Arch Linux

WezTerm is available for Arch users in the AUR; there are three options:

|What                 |Where|
|---------------------|-|
|Released Binaries    |<https://aur.archlinux.org/packages/wezterm-bin/>|
|Nightly Binaries     |<https://aur.archlinux.org/packages/wezterm-nightly-bin/>|
|Build from source    |<https://aur.archlinux.org/packages/wezterm-git/>|

## Linuxbrew Tap

If you are a [Linuxbrew](https://docs.brew.sh/Homebrew-on-Linux) user, you can install
wezterm from our tap:

```bash
$ brew tap wez/wezterm-linuxbrew
$ brew install wezterm
```

If you'd like to use a nightly build you can perform a head install:

```bash
$ brew install --HEAD wezterm
```

## Raw Linux Binary

Another option for linux is a raw binary archive.  These are the same binaries that
are built for Ubuntu but provided in a tarball.

<a href="{{ linux_raw_bin_stable }}" class="btn">Download raw Linux binaries</a>
<a href="{{ linux_raw_bin_nightly }}" class="btn">Nightly raw Linux binaries</a>


