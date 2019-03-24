#!/bin/bash
set -x

if [[ "$TRAVIS" != "" ]] ; then
  DEPLOY_ENV_TYPE="travis"
  TAG_NAME=$TRAVIS_TAG
elif [[ "$APPVEYOR" != "" ]] ; then
  DEPLOY_ENV_TYPE="appveyor"
  TAG_NAME=$APPVEYOR_REPO_TAG_NAME
else
  DEPLOY_ENV_TYPE="adhoc"
fi

TAG_NAME=${TAG_NAME:-$(git describe --tags)}
TAG_NAME=${TAG_NAME:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

HERE=$(pwd)

case $OSTYPE in
  darwin*)
    zipdir=WezTerm-macos-$DEPLOY_ENV_TYPE-$TAG_NAME
    rm -rf $zipdir $zipdir.zip
    mkdir $zipdir
    cp -r assets/macos/WezTerm.app $zipdir/
    cp target/release/wezterm $zipdir/WezTerm.app
    zip -r $zipdir.zip $zipdir
    ;;
  msys)
    zipdir=WezTerm-windows-$DEPLOY_ENV_TYPE-$TAG_NAME
    rm -rf $zipdir $zipdir.zip
    mkdir $zipdir
    cp target/release/wezterm.exe target/release/wezterm.pdb $zipdir
    7z a -tzip $zipdir.zip $zipdir
    ;;
  *)
    ;;
esac
