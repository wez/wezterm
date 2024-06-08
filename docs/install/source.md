## Installing from source

If your system isn't covered by the pre-built packages then you can build it
for yourself.  WezTerm should run on any modern unix as well as Windows 10 and
macOS.

* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html)
* Rust version 1.71 or later is required
* Build in release mode: `cargo build --release`
* Run it via either `cargo run --release --bin wezterm` or `target/release/wezterm`

You will need a collection of support libraries; the [`get-deps`](https://github.com/wez/wezterm/blob/main/get-deps) script will
attempt to install them for you.  If it doesn't know about your system,
[please contribute instructions!](https://github.com/wez/wezterm/blob/main/CONTRIBUTING.md)

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
$ git clone --depth=1 --branch=main --recursive https://github.com/wez/wezterm.git
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

### Building from source on Windows
1. install windows rust from [the rust website](https://www.rust-lang.org/tools/install) following their instructions.

- Note info on installing other tools for using rust on windows is covered at the microsoft [rust dev environment overview on windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/overview) page.

2. Install [Strawberry Perl](https://strawberryperl.com/)

- For example, downloaded the msi installer from the Strawberry Perl website, open the download and accept the defaults whe prompted to install strawberry perl in c:\Strawberry

There are many ways to make sure that strawberry perl gets used during wezterm configuration
Here is one simple way:

Open a cmd.exe terminal  (in wezterm, preferably :-)

Check which version of perl is going to be used by default:
```
> where perl
C:\Program Files\Git\usr\bin\perl.exe
```
Uh oh, I have already installed git for windows and git for windows has its own version of perl. Can't use that! (Per Wez's insructions, *only* Strawberry Perl works. So, to fix this (temporarily) set the PATH so that Strawberry perl is found first by typing in the terminal:

```
set PATH=c:\Strawberry\perl\bin;%PATH%
```
Now when we check again which perl is going to be used, we find that Strawberry Perl is going to found first:
```
> where perl
c:\Strawberry\perl\bin\perl.exe
C:\Program Files\Git\usr\bin\perl.exe

> perl --version
This is perl 5, version 38, subversion 2 (v5.38.2) built for MSWin32-x64-multi-thread
```
This is good, we are set and Strawberry perl will now be used in the build process.

3. clone and build 
```
git clone --depth=1 --branch=main --recursive https://github.com/wez/wezterm.git
cd wezterm
git submodule update --init --recursive
cargo build 
```

4. To test out the newly built wezterm, type:
```
cargo run --bin wezterm
```

And it wezterm should pop up in a new window.






