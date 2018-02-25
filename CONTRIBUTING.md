# Contributing to wezterm

Thanks for considering donating your time and energy!  I value any contribution,
even if it is just to highlight a typo.

Included here are some guidelines that can help streamline taking in your contribution.
They are just guidelines and not hard-and-fast rules.  Things will probably go faster
and smoother if you have the time and energy to read and follow these suggestions.

## Contributing Documentation

There's never enough!  Pretty much anything is fair game to improve here.

### Operating system specific installation instructions?

There are a lot of targets out there.  Today we have docs that are Ubuntu biased
and I know that there are a lot flavors of Linux.  Rather than expand the README
with intructions for those, I'd like to just bake those instructions into a script
and expand that as we go.

## Contributing code

Yes please!

If you are new to the Rust language check out https://rustbyexample.com/

### Where to find things?

The `term` directory holds the core terminal model code.  This is agnostic
of any windowing system.  If you want to add support for terminal escape
sequences and that sort of thing, you probably want to be in the `term` dir.
Keep in mind that for maximal compatibility and utility `wezterm` aims to
be compatible with the `xterm` behavior.
https://invisible-island.net/xterm/ctlseqs/ctlseqs.html is a useful resource!

The `src` directory holds the code for the `wezterm` program.  This is
the GUI renderer for the terminal model.  If you want to change something
about the GUI you want to be in the `src` dir.

### Iterating

I tend to iterate and sanity check as I develop using `cargo check`; it
will type-check your code without generating code which is much faster
than building everything in release mode:

```
$ cargo check
```

Likewise, if you want to quick check that something works, you can run it
in debug mode using:

```
$ cargo run
```

This will produce a debug instrumented binary with poor optimization.  This will
give you more detail in the backtrace produced if you run `RUST_BACKTRACE=1 cargo run`.

If you get a panic and want to examine local variables, you'll need to use gdb:

```
$ cargo build
$ gdb ./target/debug/wezterm
$ break rust_panic               # hit tab to complete the name of the panic symbol!
$ run
$ bt
```

### Please include tests to cover your changes!

This will help ensure that your contributings keep working as things change.

You can run the existing tests using:

```
$ cargo test --all
```

There are some helper classes for writing tests for terminal behavior.
Here's an example of a test to verify that the terminal contents
match expectations:

https://github.com/wez/wezterm/blob/master/term/src/test/mod.rs#L334

Please also make a point of adding comments to your tests to help
clarify the intent of the test!

### Please also include documentation if you are adding or changing behavior

This helps to keep things well-understood and working in the long term.
Don't worry if you're not a wordsmith or English isn't your first language as
I can help with that.  It is more important to capture the intent of the
feature and having this written out in English also helps when it comes
to reviewing the code.

## Submitting a Pull Request

After considering all of the above, and once you're prepared your contribution
and are ready to submit it, you'll need to create a pull request.

If you're new to GitHub pull requests, read through
https://help.github.com/articles/creating-a-pull-request/ to understand
how that process works.

### Before your submit your code

Make sure that the tests are working and that the code is correctly
formatted otherwise the continuous integration system will fail your build:

```
$ rustup component add rustfmt-preview          # you only need to do this once
$ cargo test --all
$ cargo fmt --all
```

