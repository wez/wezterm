## Installing from source

If your system isn't covered by the pre-built packages then you can build it
for yourself.  WezTerm should run on any modern unix as well as Windows 10 and
macOS.

* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html).
* Rust version 1.71 or later is required
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release --bin wezterm` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](https://github.com/wezterm/wezterm/blob/main/get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](https://github.com/wezterm/wezterm/blob/main/CONTRIBUTING.md).

If you don't plan to submit a pull request to the wezterm repo, you can
download a smaller source tarball using these steps:

```console
$ curl https://sh.rustup.rs -sSf | sh -s
$ curl -LO {{ src_stable }}
$ tar -xzf {{ src_stable_asset }}
$ cd {{ src_stable_dir }}
$ ./get-deps
$ cargo build --release
$ cargo run --release --bin wezterm -- start
```

Alternatively, use the full git repo:

```console
$ curl https://sh.rustup.rs -sSf | sh -s
$ git clone --depth=1 --branch=main --recursive https://github.com/wezterm/wezterm.git
$ cd wezterm
$ git submodule update --init --recursive
$ ./get-deps
$ cargo build --release
$ cargo run --release --bin wezterm -- start
```

**If you get an error about zlib then you most likely didn't initialize the submodules;
take a closer look at the instructions!**

### Building without Wayland support on Unix systems

By default, support for both X11 and Wayland is included on Unix systems.
If your distribution has X11 but not Wayland, then you can build WezTerm without
Wayland support by changing the `build` invocation:

```console
$ cargo build --release --no-default-features --features vendored-fonts
```

Building without X11 is not supported.

### Building on Windows

When installing Rust, you must use select the MSVC version of Rust. It is the
only supported way to build wezterm.

On Windows, instead of using `get-deps`, the only other dependency that you need is
[Strawberry Perl](https://strawberryperl.com). You must ensure that you have
your `PATH` environment set up to find that particular `perl.exe` ahead of any
other perl that you may have installed on your system. This particular version
of perl is required to build openssl on Windows.

```console
$ set PATH=c:\Strawberry\perl\bin;%PATH%
```

