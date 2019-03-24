#!/bin/bash
set -x

TRAVIS_TAG=${TRAVIS_TAG:-$(git describe --tags)}
TRAVIS_TAG=${TRAVIS_TAG:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

bash -x ./install.sh

HERE=$(pwd)

case $OSTYPE in
  darwin*)
    (cd $HOME/Applications && zip -r $HERE/WezTerm-macOS-$TRAVIS_TAG.zip WezTerm.app)
    ;;
  msys)
    zipdir=WezTerm-windows-$TRAVIS_TAG
    mkdir zipdir
    cp target/release/wezterm.exe target/release/wezterm.pdb $zipdir
    7z a -tzip $zipdir.zip $zipdir
    ;;
  *)
    ;;
esac
