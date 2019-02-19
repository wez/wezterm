#!/bin/bash
set -e
set -x

case "$OSTYPE" in
  darwin*)
    ;;
  *)
    cd ci/harfbuzz
    ./autogen.sh --prefix=$PREFIX
    make install
    ;;
esac

