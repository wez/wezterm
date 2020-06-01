---
name: Build Problem
about: Having problems building from source?
title: ''
labels: bug
assignees: ''

---

## Build Environment (please complete the following information):

 - OS: [e.g. Linux X11, Linux Wayland, macOS, Windows].  If on Linux, Please include `lsb_release -a` in your report.
 - Linux: what distro, version and architecture?  Please include `uname -a` in your report.
 - Compiler: are you using `clang`, `gcc`, `Microsoft Visual Studio` or something else?  Which version?
 - Rust version: Please include the output from `rustup show`. Best results are
   generally had with a recent stable version of the rust toolchain.

## Dependencies

Did you run the `get-deps` script to install required system dependencies?
Was it successful?

If building from the git repo, did you update the submodules?  Not doing this
is a common source of problems; see the information at
<https://wezfurlong.org/wezterm/install/source.html> for more information.

## The build output

Please include the output from running the build command:

```
cargo build --release
```
