#!/bin/bash
set -ex

RELEASE_FLAG=""

if [[ "$TRAVIS_RUST_VERSION" == "stable" ]] ; then
  cargo fmt --all -- --check
fi

if [[ "$TRAVIS_TAG" != "" ]] ; then
  RELEASE_FLAG="--release"
fi

cargo build $RELEASE_FLAG
cargo test $RELEASE_FLAG --all

