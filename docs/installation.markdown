---
title: Installation
---

## Installing a package

* Linux, macOS and Windows packages are available from [the Releases page](https://github.com/wez/wezterm/releases)
* Bleeding edge Windows package available from [Appveyor](https://ci.appveyor.com/project/wez/wezterm/build/artifacts?branch=master)

## Installing from source

* Install `rustup` to get the `rust` compiler installed on your system.
  https://www.rust-lang.org/en-US/install.html
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


