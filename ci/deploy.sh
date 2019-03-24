#!/bin/bash
set -x

TRAVIS_TAG=${TRAVIS_TAG:-$(git describe --tags)}
TRAVIS_TAG=${TRAVIS_TAG:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

./install.sh

HERE=$(pwd)

case $OSTYPE in
  darwin*)
    (cd $HOME/Applications && zip -r $HERE/WezTerm-macOS-$TRAVIS_TAG.zip WezTerm.app)
    ;;
  *)
    ;;
esac
