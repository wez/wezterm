#!/usr/bin/env bash
# You need: https://crates.io/crates/cargo-cooldown
# This script is logically equivalent to running `cargo update`
# but respects dependency cooldown settings found in the cooldown.toml
# config file
cargo cooldown update
