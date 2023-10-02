#!/bin/bash
set -x
set -e

env

command -v git || dnf install -y git

if test -d /builddir/build ; then
  COPR_BUILD_DIR=/builddir/build
fi

git config --global --add safe.directory $$PWD
git config --global --add safe.directory $$PWD/deps/freetype/freetype2
git config --global --add safe.directory $$PWD/deps/freetype/libpng
git config --global --add safe.directory $$PWD/deps/freetype/zlib
git config --global --add safe.directory $$PWD/deps/harfbuzz/harfbuzz

./ci/source-archive.sh
./ci/deploy.sh

