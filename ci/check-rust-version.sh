#!/bin/bash

min_rust=(1 53 0)
rust_ver=()

parse_rustc_version() {
  local IFS
  IFS=' '
  local FIELDS
  read -ra FIELDS <<< $(rustc --version)
  IFS='.'
  read -ra rust_ver <<< "${FIELDS[1]}"
}

check_rust_version() {
  parse_rustc_version
  # rust_ver=(1 46 0) for testing purposes

  if [[ "${rust_ver[0]}" -gt "${min_rust[0]}" ]] ; then
    return 0
  fi
  if [[ "${rust_ver[0]}" -lt "${min_rust[0]}" ]] ; then
    return 1
  fi
  if [[ "${rust_ver[1]}" -gt "${min_rust[1]}" ]] ; then
    return 0
  fi
  if [[ "${rust_ver[1]}" -lt "${min_rust[1]}" ]] ; then
    return 1
  fi
  if [[ "${rust_ver[2]}" -gt "${min_rust[2]}" ]] ; then
    return 0
  fi
  if [[ "${rust_ver[2]}" -lt "${min_rust[2]}" ]] ; then
    return 1
  fi

  return 0
}

if ! check_rust_version ; then
  rust_ver=$(IFS=. ; echo "${rust_ver[*]}")
  min_rust=$(IFS=. ; echo "${min_rust[*]}")
  echo "Installed rustc version $rust_ver is less than required $min_rust"
  echo
  echo "Using rustup to manage your installed version of Rust is recommended"
  echo "https://www.rust-lang.org/en-US/install.html"
  echo
  echo "See https://wezfurlong.org/wezterm/install/source.html for complete"
  echo "installation instructions for wezterm"
  exit 1
fi

exit 0
