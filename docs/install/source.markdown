## Installing from source

If your system isn't covered by the pre-built packages then you can build it
for yourself.  WezTerm should run on any modern unix as well as Windows 10 and
macOS.

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

**If you get an error about zlib then you most likely didn't initialize the submodules;
take a closer look at the instructions!**
