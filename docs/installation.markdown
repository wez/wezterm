---
title: Installation
---

## {% octicon cloud-download height:24 %} Installing a pre-built package on Windows

Windows 10 or later is required to run WezTerm.

{% for asset in site.github.latest_release.assets %}
  {% if asset.name contains 'azure' and asset.name contains 'windows' %}
<a href="{{ asset.browser_download_url }}" class="btn">{% octicon cloud-download %} Download for Windows</a>
1. Download <a href="{{ asset.browser_download_url }}">{{ asset.name }}</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](configuration.html)
  {% endif %}
{% endfor %}

## {% octicon cloud-download height:24 %} Installing a pre-built package on macOS

The CI system builds the package on macOS Mojave (10.14).  It may run on earlier
versions of macOS, but that has not been tested.

{% for asset in site.github.latest_release.assets %}
  {% if asset.name contains 'azure' and asset.name contains 'macos' %}
<a href="{{ asset.browser_download_url }}" class="btn">{% octicon cloud-download %} Download for macOS</a>
1. Download <a href="{{ asset.browser_download_url }}">{{ asset.name }}</a>
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder
3. First time around, you may need to right click and select `Open` to allow launching
   the application that your just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI
4. Configuration instructions can be [found here](configuration.html)
  {% endif %}
{% endfor %}

## {% octicon cloud-download height:24 %} Installing a pre-built package on Ubuntu

The CI system builds a `.deb` file on Ubuntu 16.04.  It may be compatible with other
debian style systems.

{% for asset in site.github.latest_release.assets %}
  {% if asset.name contains '.deb' %}

<a href="{{ asset.browser_download_url }}" class="btn">{% octicon cloud-download %} Download for Ubuntu</a>
* <tt>curl -LO <a href="{{ asset.browser_download_url }}">{{ asset.browser_download_url }}</a></tt>
* `sudo dpkg -i {{ asset.name }}`
* The package installs `/usr/bin/wezterm`
* Configuration instructions can be [found here](configuration.html)
  {% endif %}
{% endfor %}

## {% octicon beaker height:24 %} Nightly builds

Bleeding edge nightly pre-release builds may be available from [the releases page](https://github.com/wez/wezterm/releases).

## {% octicon git-branch height:24 %} Installing from source

* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html)
* Rust version 1.35 or later is required
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](https://github.com/wez/wezterm/blob/master/get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](https://github.com/wez/wezterm/blob/master/CONTRIBUTING.md)

```
$ curl https://sh.rustup.rs -sSf | sh -s
$ git clone --depth=1 --branch=master --recursive https://github.com/wez/wezterm.git
$ cd wezterm
$ git submodule update --init
$ sudo ./get-deps
$ cargo build --release
$ cargo run --release -- start
```

