#!/bin/bash
set -x
set -e

TARGET_DIR=${1:-target}

TAG_NAME=${TAG_NAME:-$(git describe --tags)}
TAG_NAME=${TAG_NAME:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

HERE=$(pwd)

case $OSTYPE in
  darwin*)
    zipdir=WezTerm-macos-$TAG_NAME
    if [[ "$BUILD_REASON" == "Schedule" ]] ; then
      zipname=WezTerm-macos-nightly.zip
    else
      zipname=$zipdir.zip
    fi
    rm -rf $zipdir $zipname
    mkdir $zipdir
    cp -r assets/macos/WezTerm.app $zipdir/
    cp -r assets/shell-integration/* $zipdir/WezTerm.app
    cp $TARGET_DIR/release/wezterm $zipdir/WezTerm.app
    cp $TARGET_DIR/release/wezterm-mux-server $zipdir/WezTerm.app
    cp $TARGET_DIR/release/wezterm-gui $zipdir/WezTerm.app
    cp $TARGET_DIR/release/strip-ansi-escapes $zipdir/WezTerm.app
    zip -r $zipname $zipdir

    SHA256=$(shasum -a 256 $zipname | cut -d' ' -f1)
    sed -e "s/@TAG@/$TAG_NAME/g" -e "s/@SHA256@/$SHA256/g" < ci/wezterm-homebrew-macos.rb.template > wezterm.rb
    cat wezterm.rb

    ;;
  msys)
    zipdir=WezTerm-windows-$TAG_NAME
    if [[ "$BUILD_REASON" == "Schedule" ]] ; then
      zipname=WezTerm-windows-nightly.zip
      instname=WezTerm-nightly-setup
    else
      zipname=$zipdir.zip
      instname=WezTerm-${TAG_NAME}-setup
    fi
    rm -rf $zipdir $zipname
    mkdir $zipdir
    cp $TARGET_DIR/release/wezterm.exe \
      $TARGET_DIR/release/wezterm-mux-server.exe \
      $TARGET_DIR/release/wezterm-gui.exe \
      $TARGET_DIR/release/strip-ansi-escapes.exe \
      $TARGET_DIR/release/wezterm.pdb \
      assets/windows/conhost/conpty.dll \
      assets/windows/conhost/OpenConsole.exe \
      $zipdir
    mkdir $zipdir/mesa
    cp $TARGET_DIR/release/mesa/opengl32.dll \
        $zipdir/mesa
    7z a -tzip $zipname $zipdir
    iscc.exe -DMyAppVersion=${TAG_NAME#nightly} -F${instname} ci/windows-installer.iss
    ;;
  linux-gnu)
    distro=$(lsb_release -is)
    distver=$(lsb_release -rs)
    case "$distro" in
      *Fedora*|*CentOS*)
        WEZTERM_RPM_VERSION=$(echo ${TAG_NAME#nightly-} | tr - _)
        cat > wezterm.spec <<EOF
Name: wezterm
Version: ${WEZTERM_RPM_VERSION}
Release: 1%{?dist}
Packager: Wez Furlong <wez@wezfurlong.org>
License: MIT
URL: https://wezfurlong.org/wezterm/
Summary: Wez's Terminal Emulator.
Requires: dbus, fontconfig, openssl, libxcb, libxkbcommon, libxkbcommon-x11, libwayland-client, libwayland-egl, libwayland-cursor, mesa-libEGL, xcb-util-keysyms, xcb-util-wm

%description
wezterm is a terminal emulator with support for modern features
such as fonts with ligatures, hyperlinks, tabs and multiple
windows.

%build
echo "Doing the build bit here"

%install
set -x
cd ${HERE}
mkdir -p %{buildroot}/usr/bin %{buildroot}/etc/profile.d
install -Dsm755 target/release/wezterm -t %{buildroot}/usr/bin
install -Dsm755 target/release/wezterm-mux-server -t %{buildroot}/usr/bin
install -Dsm755 target/release/wezterm-gui -t %{buildroot}/usr/bin
install -Dsm755 target/release/strip-ansi-escapes -t %{buildroot}/usr/bin
install -Dm644 assets/shell-integration/* -t %{buildroot}/etc/profile.d
install -Dm644 assets/icon/terminal.png %{buildroot}/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
install -Dm644 assets/wezterm.desktop %{buildroot}/usr/share/applications/org.wezfurlong.wezterm.desktop
install -Dm644 assets/wezterm.appdata.xml %{buildroot}/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml

%files
/usr/bin/wezterm
/usr/bin/wezterm-gui
/usr/bin/wezterm-mux-server
/usr/bin/strip-ansi-escapes
/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
/usr/share/applications/org.wezfurlong.wezterm.desktop
/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml
/etc/profile.d/*
EOF

        /usr/bin/rpmbuild -bb --rmspec wezterm.spec --verbose

        ;;
      Ubuntu*|Debian*)
        rm -rf pkg
        mkdir -p pkg/debian/usr/bin pkg/debian/DEBIAN pkg/debian/usr/share/{applications,wezterm}
        cat > pkg/debian/DEBIAN/control <<EOF
Package: wezterm
Version: ${TAG_NAME#nightly-}
Architecture: $(dpkg-architecture -q DEB_BUILD_ARCH_CPU)
Maintainer: Wez Furlong <wez@wezfurlong.org>
Section: utils
Priority: optional
Homepage: https://wezfurlong.org/wezterm/
Description: Wez's Terminal Emulator.
 wezterm is a terminal emulator with support for modern features
 such as fonts with ligatures, hyperlinks, tabs and multiple
 windows.
Depends: libc6, libegl-mesa0, libxcb-icccm4, libxcb-ewmh2, libxcb-keysyms1, libxcb-xkb1, libxkbcommon0, libxkbcommon-x11-0, libfontconfig1, xdg-utils, libxcb-render0, libxcb-shape0, libx11-6, libegl1
EOF
        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm-mux-server
        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm-gui
        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm
        install -Dsm755 -t pkg/debian/usr/bin target/release/strip-ansi-escapes
        install -Dm644 assets/icon/terminal.png pkg/debian/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
        install -Dm644 assets/wezterm.desktop pkg/debian/usr/share/applications/org.wezfurlong.wezterm.desktop
        install -Dm644 assets/wezterm.appdata.xml pkg/debian/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml
        install -Dm644 assets/shell-integration/* -t pkg/debian/etc/profile.d
        if [[ "$BUILD_REASON" == "Schedule" ]] ; then
          debname=wezterm-nightly.$distro$distver
        else
          debname=wezterm-$TAG_NAME.$distro$distver
        fi
        fakeroot dpkg-deb --build pkg/debian $debname.deb
        tar cJf $debname.tar.xz -C pkg/debian/usr/bin wezterm
        rm -rf pkg
      ;;
    esac
    ;;
  *)
    ;;
esac
