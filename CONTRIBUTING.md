# Contributing to wezterm

Thanks for considering donating your time and energy!  I value any contribution,
even if it is just to highlight a typo.

Included here are some guidelines that can help streamline taking in your contribution.
They are just guidelines and not hard-and-fast rules. Things will probably go faster
and smoother if you have the time and energy to read and follow these suggestions.

## Contributing Documentation

There's never enough!  Pretty much anything is fair game to improve here.

### Running the doc build yourself

To check your documentation additions, you can optionally build the docs yourself and see how the changes will look on the webpage. 

To serve them up, and then automatically rebuild and refresh the docs in your browser, run:
```console
$ ci/build-docs.sh serve
```
And then click on the URL that it prints out after it has performed the first build.

Any arguments passed to `build-docs.sh` are passed down to the underlying `mkdocs` utility.

Look at [mkdocs serve](https://www.mkdocs.org/user-guide/cli/#mkdocs-serve) for more information on additional parameters.

### Operating system specific installation instructions?

There are a lot of targets out there.  Today we have docs that are Ubuntu biased
and I know that there are a lot of flavors of Linux. Rather than expand the README
with instructions for those, please translate the instructions into steps that
can be run in the [`get-deps`](https://github.com/wezterm/wezterm/blob/master/get-deps)
script.

## Contributing code

Yes please!

If you are new to the Rust language check out <https://doc.rust-lang.org/rust-by-example/>.

### Building from source

To build wezterm from source, you will need a local Rust toolchain, and a few platform-specific dependencies.
Follow the [Install from Source](https://wezfurlong.org/wezterm/install/source.html) guide to get started!

Some platforms like Windows have a few specific steps, make sure to check the dedicated sections in the guide.

### Where to find things?

The `term` directory holds the core terminal model code. This is agnostic
of any windowing system. If you want to add support for terminal escape
sequences and that sort of thing, you probably want to be in the `term` directory.
Keep in mind that for maximal compatibility and utility `wezterm` aims to
be compatible with the `xterm` behavior.
https://invisible-island.net/xterm/ctlseqs/ctlseqs.html is a useful resource!

The `src` directory holds the code for the `wezterm` program. This is
the GUI renderer for the terminal model.  If you want to change something
about the GUI you want to be in the `src` dir.

### Iterating

I tend to iterate and sanity check as I develop using `cargo check`; it
will type-check your code without generating code which is much faster
than building everything in release mode:

```console
$ cargo check
```

Likewise, if you want to quickly check that something works, you can run it
in debug mode using:

```console
$ cargo run
```

This will produce a debug-instrumented binary with poor optimization. This will
give you more detail in the backtrace produced if you run `RUST_BACKTRACE=1 cargo run`.

If you get a panic and want to examine local variables, you'll need to use gdb:

```console
$ cargo build
$ gdb ./target/debug/wezterm
$ break rust_panic               # hit tab to complete the name of the panic symbol!
$ run
$ bt
```

Starting WezTerm with `wezterm-gui start --always-new-process` is useful to ensure Mux logs are not
hidden in an background process started in an earlier test.

Start WezTerm with `wezterm-gui --config-file ./test-conf.lua ……` to test a custom config file.


### Please include tests to cover your changes!

This will help ensure that your contributions keep working as things change.

You can run the existing tests using:

```console
$ cargo test --all
```

There are some helper classes for writing tests for terminal behavior.
Here's [an example of a test to verify that the terminal contents
match expectations](https://github.com/wezterm/wezterm/blob/fd532a8c2fb3b56593597cf8be1775da1feda0a3/term/src/test/mod.rs#L314).

Please also make a point of adding comments to your tests to help
clarify the intent of the test!

### Testing in a NixOS VM

If you need to test WezTerm in a clean desktop environment (e.g. to reproduce a
display server bug or verify a desktop integration), you can use the provided
NixOS VM configurations.

Two desktop variants are available:
- GNOME (`testing-on-gnome`)
- KDE Plasma (`testing-on-plasma`).

**Prerequisites:** Nix is installed, with flakes enabled (`experimental-features = nix-command flakes`).

**Workflow:**

```console
$ cargo build                                            # build wezterm locally
$ nixos-rebuild build-vm --flake ./nix#testing-on-plasma # build the Plasma VM image
$ REPO=$PWD ./result/bin/run-nixos-vm                    # start the VM
```

> [!NOTE]
> You might need to tweak desktop settings (e.g. the keyboard layout) on first VM start.
>
> The VM state is stored in `nixos.qcow2` in the current working directory on the host.

Inside the VM, open a terminal (e.g. _Console_ on GNOME, _Konsole_ on Plasma), then:

```console
$ cd repo                      # go into ~/repo
$ ./target/debug/wezterm-gui   # run wezterm for your tests
```

The host repository is mounted into the VM at `/home/dev/repo` via a 9p shared directory,
so changes on the host (rebuilds, test config file, ..) are immediately visible inside the VM.

Keep the VM running and make your changes locally, re-build wezterm on your host and kill/start
wezterm again in the VM for your tests.

> [!WARNING]
> The host repo is supposed to be mounted in read-write but I (@bew) don't know why changes in the
> repo are not actually propagated to the host. (PR welcome!)

### Please also include documentation if you are adding or changing behavior

This helps to keep things well-understood and working in the long term.
Don't worry if you're not a wordsmith or English isn't your first language as
I can help with that. It is more important to capture the intent of the
feature and having this written out in English also helps when it comes
to reviewing the code.

## Submitting a Pull Request

After considering all of the above, and once you've prepared your contribution
and are ready to submit it, you'll need to create a pull request.

If you're new to GitHub Pull Requests, read through
https://help.github.com/articles/creating-a-pull-request/ to understand
how the process works.

### Before you submit your code

Make sure that the tests are working and that the code is correctly
formatted otherwise the continuous integration system will fail your build:

```console
$ rustup component add rustfmt-preview          # you only need to do this once
$ cargo test --all
$ cargo fmt --all
```

