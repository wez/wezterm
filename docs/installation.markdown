---
title: Installation
---

{% for r in site.github.releases %}
{%   if r.prerelease and prerelease == nil %}
{%     assign prerelease = r %}
{%   elsif r.prerelease == false and release == nil %}
{%     assign release = r %}
{%   endif %}
{% endfor %}
{% for asset in release.assets %}
{%   if asset.name contains 'windows' and windows_stable == nil %}
{%     assign windows_stable = asset.browser_download_url %}
{%   endif %}
{%   if asset.name contains 'macos' and macos_stable == nil %}
{%     assign macos_stable = asset.browser_download_url %}
{%   endif %}
{%   if asset.name contains '.deb' and deb_stable == nil %}
{%     assign deb_stable = asset.browser_download_url %}
{%   endif %}
{% endfor %}
{% for asset in prerelease.assets %}
{%   if asset.name contains 'windows' and windows_pre == nil %}
{%     assign windows_pre = asset.browser_download_url %}
{%   endif %}
{%   if asset.name contains 'macos' and macos_pre == nil %}
{%     assign macos_pre = asset.browser_download_url %}
{%   endif %}
{%   if asset.name contains '.deb' and deb_pre == nil %}
{%     assign deb_pre = asset.browser_download_url %}
{%   endif %}
{% endfor %}


## {% octicon cloud-download height:24 %} Installing a pre-built package on Windows

Windows 10 or later is required to run WezTerm.

<a href="{{ windows_stable }}" class="btn">{% octicon cloud-download %} Download for Windows</a>
<a href="{{ windows_pre }}" class="btn">{% octicon beaker %} Nightly for Windows</a>
1. Download <a href="{{ windows_stable }}">Release</a>
2. Extract the zipfile and double-click `wezterm.exe` to run the UI
3. Configuration instructions can be [found here](configuration.html)

## {% octicon cloud-download height:24 %} Installing a pre-built package on macOS

The CI system builds the package on macOS Mojave (10.14).  It may run on earlier
versions of macOS, but that has not been tested.

<a href="{{ macos_stable }}" class="btn">{% octicon cloud-download %} Download for macOS</a>
<a href="{{ macos_pre }}" class="btn">{% octicon beaker %} Nightly for macOS</a>
1. Download <a href="{{ macos_stable }}">Release</a>
2. Extract the zipfile and drag the `WezTerm.app` bundle to your `Applications` folder
3. First time around, you may need to right click and select `Open` to allow launching
   the application that your just downloaded from the internet.
3. Subsequently, a simple double-click will launch the UI
4. Configuration instructions can be [found here](configuration.html)

## {% octicon cloud-download height:24 %} Installing a pre-built package on Ubuntu

The CI system builds a `.deb` file on Ubuntu 16.04.  It may be compatible with other
debian style systems.

<a href="{{ deb_stable }}" class="btn">{% octicon cloud-download %} Download for Ubuntu</a>
{% if deb_pre %}
<a href="{{ deb_pre }}" class="btn">{% octicon beaker %} Nightly for Ubuntu</a>
{% endif %}
* <tt>curl -LO <a href="{{ deb_stable }}">{{ deb_stable }}</a></tt>
* `sudo dpkg -i {{ asset.name }}`
* The package installs `/usr/bin/wezterm`
* Configuration instructions can be [found here](configuration.html)

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

