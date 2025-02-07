#!/usr/bin/env bash

min_rust="1.71.0"
rust_ver="$(rustc --version | cut -d' ' -f2)"

check_rust_version() {
  ver=$(printf "%s\n%s\n" "$min_rust" "$rust_ver" | sort --version-sort | head -n1)
  if test "$ver" = "$min_rust"; then
    return 0
  else
    return 1
  fi
}

if ! check_rust_version ; then
  echo "Installed rustc version '$rust_ver' is less than required '$min_rust'"
  echo
  echo "Check if your OS provides newer version of Rust, if not"
  echo "use rustup to manage installed versions of Rust"
  echo "https://www.rust-lang.org/en-US/install.html"
  echo
  echo "See https://wezterm.org/install/source.html for complete"
  echo "installation instructions for wezterm"
  exit 1
fi

exit 0
