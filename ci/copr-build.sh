#!/bin/bash
set -x
set -e


command -v git || dnf install -y git

git config --global --add safe.directory $PWD
git config --global --add safe.directory $PWD/deps/freetype/freetype2
git config --global --add safe.directory $PWD/deps/freetype/libpng
git config --global --add safe.directory $PWD/deps/freetype/zlib
git config --global --add safe.directory $PWD/deps/harfbuzz/harfbuzz

./ci/source-archive.sh
./ci/deploy.sh

