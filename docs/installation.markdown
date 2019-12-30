
## Installing on Windows

Windows 10 or later is required to run WezTerm.

<a href="{{ windows_stable }}" class="btn">Download for Windows</a>
<a href="{{ windows_pre }}" class="btn">Nightly for Windows</a>
1. Download <a href="{{ windows_stable }}">Release</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](configuration.html)

## Installing on macOS

The CI system builds the package on macOS Mojave (10.14).  It may run on earlier
versions of macOS, but that has not been tested.

<a href="{{ macos_stable }}" class="btn">Download for macOS</a>
<a href="{{ macos_pre }}" class="btn">Nightly for macOS</a>
1. Download <a href="{{ macos_stable }}">Release</a>
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder
3. First time around, you may need to right click and select `Open` to allow launching
   the application that your just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI
4. Configuration instructions can be [found here](configuration.html)

## Installing on Ubuntu

The CI system builds a `.deb` file on Ubuntu 16.04.  It is compatible with other
debian style systems, including Debian 9 (Stretch) and later versions.

<a href="{{ ubuntu_stable }}" class="btn">Download for Ubuntu</a>
<a href="{{ ubuntu_pre }}" class="btn">Nightly for Ubuntu</a>

```bash
curl -LO {{ ubuntu_stable }}
sudo apt install -y ./{{ ubuntu_stable_asset }}
```

* The package installs `/usr/bin/wezterm` and `/usr/share/applications/wezterm.desktop`
* Configuration instructions can be [found here](configuration.html)

## Installing Fedora

The CI system builds an `.rpm` file on Fedora 31.

<a href="{{ fedora_stable }}" class="btn">Download for Fedora</a>
<a href="{{ fedora_pre }}" class="btn">Nightly for Fedora</a>

```bash
curl -LO {{ fedora_stable }}
sudo dnf install -y ./{{ fedora_stable_asset }}
```

* The package installs `/usr/bin/wezterm` and `/usr/share/applications/wezterm.desktop`
* Configuration instructions can be [found here](configuration.html)

## Installing from source

* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html)
* Rust version 1.39 or later is required
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](https://github.com/wez/wezterm/blob/master/get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](https://github.com/wez/wezterm/blob/master/CONTRIBUTING.md)

If you don't plan to submit a pull request to the wezterm repo, you can
download a smaller source tarball using these steps:

```bash
curl https://sh.rustup.rs -sSf | sh -s
curl -LO {{ source_stable }}
tar -xzf {{ source_stable_asset }}
cd {{ source_stable_dir }}
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
