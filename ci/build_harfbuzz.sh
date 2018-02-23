#!/bin/sh
set -e
set -x

cd ci/harfbuzz
./autogen.sh --prefix=$PREFIX
make install

