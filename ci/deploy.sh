#!/bin/bash
set -x

./install.sh

HERE=$(pwd)

case $OSTYPE in
  darwin*)
    (cd $HOME/Applications && zip -r $HERE/WezTerm-macOS.zip WezTerm.app)
    ;;
  *)
    ;;
esac
