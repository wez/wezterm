#!/bin/bash
set -x
set -e

if [[ "$TRAVIS" != "" ]] ; then
  DEPLOY_ENV_TYPE="travis"
  TAG_NAME=$TRAVIS_TAG
elif [[ "$APPVEYOR" != "" ]] ; then
  DEPLOY_ENV_TYPE="appveyor"
  TAG_NAME=$APPVEYOR_REPO_TAG_NAME
elif [[ "$TF_BUILD" != "" ]] ; then
  DEPLOY_ENV_TYPE="azure"
else
  DEPLOY_ENV_TYPE="adhoc"
fi

TAG_NAME=${TAG_NAME:-$(git describe --tags)}
TAG_NAME=${TAG_NAME:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

# Emit the tagname variable for azure to pick up
echo "##vso[task.setvariable variable=wezterm.tagname]$TAG_NAME"

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
  linux-gnu)
    case `lsb_release -ds` in
      Ubuntu*|Debian*)
        rm -rf pkg
        mkdir -p pkg/debian/usr/bin pkg/debian/DEBIAN
        cat > pkg/debian/DEBIAN/control <<EOF
Package: wezterm
Version: ${TAG_NAME}
Architecture: amd64
Maintainer: Wez Furlong <wez@wezfurlong.org>
Section: utils
Priority: optional
Homepage: https://github.com/wez/wezterm
Description: Wez's Terminal Emulator.
 wezterm is a terminal emulator with support for modern features
 such as fonts with ligatures, hyperlinks, tabs and multiple
 windows.
Depends: libc6, libegl-mesa0, libxcb-icccm4, libxcb-ewmh2, libxcb-keysyms1, libxcb-xkb1, libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, xdg-utils, libxcb-render0, libxcb-shape0, libx11-6, libegl1
EOF
        cp target/release/wezterm pkg/debian/usr/bin
        fakeroot dpkg-deb --build pkg/debian wezterm-$TAG_NAME.deb
        rm -rf pkg
      ;;
    esac

    ;;
  *)
    ;;
esac
