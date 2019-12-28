---
name: Build Problem
about: Having problems building from source?
title: ''
labels: bug
assignees: ''

---

## Build Environment (please complete the following information):

 - OS: [e.g. Linux X11, Linux Wayland, macOS, Windows]
 - Linux: what distro, version and architecture?  Please include `uname -a` in your report.
 - Compiler: are you using `clang`, `gcc`, `Microsoft Visual Studio` or something else?  Which version?
 - Rust version: Please include the output from `rustup show`

## Dependencies

Did you run the `get-deps` script to install required system dependencies?
Was it successful?

## The build output

Please include the output from running the build command:

```
cargo build --release
```
