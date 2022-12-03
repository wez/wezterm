#!/bin/bash
set -x
set -e

TARGET_DIR=${1:-target}

TAG_NAME=${TAG_NAME:-$(git -c "core.abbrev=8" show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S")}

HERE=$(pwd)

if test -z "${SUDO+x}" && hash sudo 2>/dev/null; then
  SUDO="sudo"
fi

if test -e /etc/os-release; then
  . /etc/os-release
fi


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
    # Omit MetalANGLE for now; it's a bit laggy compared to CGL,
    # and on M1/Big Sur, CGL is implemented in terms of Metal anyway
    rm $zipdir/WezTerm.app/*.dylib
    mkdir -p $zipdir/WezTerm.app/Contents/MacOS
    mkdir -p $zipdir/WezTerm.app/Contents/Resources
    cp -r assets/shell-integration/* $zipdir/WezTerm.app/Contents/Resources

    for bin in wezterm wezterm-mux-server wezterm-gui strip-ansi-escapes ; do
      # If the user ran a simple `cargo build --release`, then we want to allow
      # a single-arch package to be built
      if [[ -f target/release/$bin ]] ; then
        cp target/release/$bin $zipdir/WezTerm.app/Contents/MacOS/$bin
      else
        # The CI runs `cargo build --target XXX --release` which means that
        # the binaries will be deployed in `target/XXX/release` instead of
        # the plain path above.
        # In that situation, we have two architectures to assemble into a
        # Universal ("fat") binary, so we use the `lipo` tool for that.
        lipo target/*/release/$bin -output $zipdir/WezTerm.app/Contents/MacOS/$bin -create
      fi
    done

    set +x
    if [ -n "$MACOS_TEAM_ID" ] ; then
      MACOS_PW=$(echo $MACOS_CERT_PW | base64 --decode)
      echo "pw sha"
      echo $MACOS_PW | shasum

      # Remove pesky additional quotes from default-keychain output
      def_keychain=$(eval echo $(security default-keychain -d user))
      echo "Default keychain is $def_keychain"
      echo "Speculative delete of build.keychain"
      security delete-keychain build.keychain || true
      echo "Create build.keychain"
      security create-keychain -p "$MACOS_PW" build.keychain
      echo "Make build.keychain the default"
      security default-keychain -d user -s build.keychain
      echo "Unlock build.keychain"
      security unlock-keychain -p "$MACOS_PW" build.keychain
      echo "Import .p12 data"
      echo $MACOS_CERT | base64 --decode > /tmp/certificate.p12
      echo "decoded sha"
      shasum /tmp/certificate.p12
      security import /tmp/certificate.p12 -k build.keychain -P "$MACOS_PW" -T /usr/bin/codesign
      rm /tmp/certificate.p12
      echo "Grant apple tools access to build.keychain"
      security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$MACOS_PW" build.keychain
      echo "Codesign"
      /usr/bin/codesign --keychain build.keychain --force --options runtime \
        --entitlements ci/macos-entitlement.plist --deep --sign "$MACOS_TEAM_ID" $zipdir/WezTerm.app/
      echo "Restore default keychain"
      security default-keychain -d user -s $def_keychain
      echo "Remove build.keychain"
      security delete-keychain build.keychain || true
    fi

    set -x
    zip -r $zipname $zipdir
    set +x

    if [ -n "$MACOS_TEAM_ID" ] ; then
      echo "Notarize"
      xcrun notarytool submit $zipname --wait --team-id "$MACOS_TEAM_ID" --apple-id "$MACOS_APPLEID" --password "$MACOS_APP_PW"
    fi
    set -x

    SHA256=$(shasum -a 256 $zipname | cut -d' ' -f1)
    sed -e "s/@TAG@/$TAG_NAME/g" -e "s/@SHA256@/$SHA256/g" < ci/wezterm-homebrew-macos.rb.template > wezterm.rb

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
      assets/windows/angle/libEGL.dll \
      assets/windows/angle/libGLESv2.dll \
      $zipdir
    mkdir $zipdir/mesa
    cp $TARGET_DIR/release/mesa/opengl32.dll \
        $zipdir/mesa
    7z a -tzip $zipname $zipdir
    iscc.exe -DMyAppVersion=${TAG_NAME#nightly} -F${instname} ci/windows-installer.iss
    ;;
  linux-gnu|linux)
    distro=$(lsb_release -is 2>/dev/null || sh -c "source /etc/os-release && echo \$NAME")
    distver=$(lsb_release -rs 2>/dev/null || sh -c "source /etc/os-release && echo \$VERSION_ID")
    case "$distro" in
      *Fedora*|*CentOS*|*SUSE*)
        WEZTERM_RPM_VERSION=$(echo ${TAG_NAME#nightly-} | tr - _)
        distroid=$(sh -c "source /etc/os-release && echo \$ID" | tr - _)
        distver=$(sh -c "source /etc/os-release && echo \$VERSION_ID" | tr - _)
        cat > wezterm.spec <<EOF
Name: wezterm
Version: ${WEZTERM_RPM_VERSION}
Release: 1.${distroid}${distver}
Packager: Wez Furlong <wez@wezfurlong.org>
License: MIT
URL: https://wezfurlong.org/wezterm/
Summary: Wez's Terminal Emulator.
%if 0%{?suse_version}
Requires: dbus-1, fontconfig, openssl, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-egl1, libwayland-cursor0, Mesa-libEGL1, libxcb-keysyms1, libxcb-ewmh2, libxcb-icccm4
%else
Requires: dbus, fontconfig, openssl, libxcb, libxkbcommon, libxkbcommon-x11, libwayland-client, libwayland-egl, libwayland-cursor, mesa-libEGL, xcb-util-keysyms, xcb-util-wm
%endif

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
install -Dm755 assets/open-wezterm-here -t %{buildroot}/usr/bin
install -Dsm755 target/release/wezterm -t %{buildroot}/usr/bin
install -Dsm755 target/release/wezterm-mux-server -t %{buildroot}/usr/bin
install -Dsm755 target/release/wezterm-gui -t %{buildroot}/usr/bin
install -Dsm755 target/release/strip-ansi-escapes -t %{buildroot}/usr/bin
install -Dm644 assets/shell-integration/* -t %{buildroot}/etc/profile.d
install -Dm644 assets/shell-completion/zsh %{buildroot}/usr/share/zsh/site-functions/_wezterm
install -Dm644 assets/shell-completion/bash %{buildroot}/etc/bash_completion.d/wezterm
install -Dm644 assets/icon/terminal.png %{buildroot}/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
install -Dm644 assets/wezterm.desktop %{buildroot}/usr/share/applications/org.wezfurlong.wezterm.desktop
install -Dm644 assets/wezterm.appdata.xml %{buildroot}/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml
install -Dm644 assets/wezterm-nautilus.py %{buildroot}/usr/share/nautilus-python/extensions/wezterm-nautilus.py

%files
/usr/bin/open-wezterm-here
/usr/bin/wezterm
/usr/bin/wezterm-gui
/usr/bin/wezterm-mux-server
/usr/bin/strip-ansi-escapes
/usr/share/zsh/site-functions/_wezterm
/etc/bash_completion.d/wezterm
/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
/usr/share/applications/org.wezfurlong.wezterm.desktop
/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml
/usr/share/nautilus-python/extensions/wezterm-nautilus.py*
/etc/profile.d/*
EOF

        /usr/bin/rpmbuild -bb --rmspec wezterm.spec --verbose

        ;;
      Ubuntu*|Debian*)
        rm -rf pkg
        mkdir -p pkg/debian/usr/bin pkg/debian/DEBIAN pkg/debian/usr/share/{applications,wezterm}
        cat > pkg/debian/control <<EOF
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
Provides: x-terminal-emulator
Source: https://wezfurlong.org/wezterm/
EOF

        cat > pkg/debian/postinst <<EOF
#!/bin/sh
set -e
if [ "\$1" = "configure" ] ; then
        update-alternatives --install /usr/bin/x-terminal-emulator x-terminal-emulator /usr/bin/open-wezterm-here 20
fi
EOF

        cat > pkg/debian/prerm <<EOF
#!/bin/sh
set -e
if [ "\$1" = "remove" ]; then
	update-alternatives --remove x-terminal-emulator /usr/bin/open-wezterm-here
fi
EOF

        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm-mux-server
        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm-gui
        install -Dsm755 -t pkg/debian/usr/bin target/release/wezterm
        install -Dm755 -t pkg/debian/usr/bin assets/open-wezterm-here
        install -Dsm755 -t pkg/debian/usr/bin target/release/strip-ansi-escapes

        deps=$(cd pkg && dpkg-shlibdeps -O -e debian/usr/bin/*)
        mv pkg/debian/postinst pkg/debian/DEBIAN/postinst
        chmod 0755 pkg/debian/DEBIAN/postinst
        mv pkg/debian/prerm pkg/debian/DEBIAN/prerm
        chmod 0755 pkg/debian/DEBIAN/prerm
        mv pkg/debian/control pkg/debian/DEBIAN/control
        echo $deps | sed -e 's/shlibs:Depends=/Depends: /' >> pkg/debian/DEBIAN/control
        cat pkg/debian/DEBIAN/control

        install -Dm644 assets/icon/terminal.png pkg/debian/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
        install -Dm644 assets/wezterm.desktop pkg/debian/usr/share/applications/org.wezfurlong.wezterm.desktop
        install -Dm644 assets/wezterm.appdata.xml pkg/debian/usr/share/metainfo/org.wezfurlong.wezterm.appdata.xml
        install -Dm644 assets/wezterm-nautilus.py pkg/debian/usr/share/nautilus-python/extensions/wezterm-nautilus.py
        install -Dm644 assets/shell-completion/bash pkg/debian/usr/share/bash-completion/completions/wezterm
        install -Dm644 assets/shell-completion/zsh pkg/debian/usr/share/zsh/functions/Completion/Unix/_wezterm
        install -Dm644 assets/shell-integration/* -t pkg/debian/etc/profile.d
        if [[ "$BUILD_REASON" == "Schedule" ]] ; then
          debname=wezterm-nightly.$distro$distver
        else
          debname=wezterm-$TAG_NAME.$distro$distver
        fi
        fakeroot dpkg-deb --build pkg/debian $debname.deb

        if [[ "$BUILD_REASON" != '' ]] ; then
          $SUDO apt-get install ./$debname.deb
        fi

        mv pkg/debian pkg/wezterm
        tar cJf $debname.tar.xz -C pkg wezterm
        rm -rf pkg
      ;;
    esac
    ;;
  linux-musl)
    case $ID in
      alpine)
        export SUDO=''
        abuild-keygen -a -n -b 8192
        pkgver="${TAG_NAME#nightly-}"
        cat > APKBUILD <<EOF
# Maintainer: Wez Furlong <wez@wezfurlong.org>
pkgname=wezterm
pkgver=$(echo "$pkgver" | cut -d'-' -f1-2 | tr - .)
_pkgver=$pkgver
pkgrel=0
pkgdesc="A GPU-accelerated cross-platform terminal emulator and multiplexer written in Rust"
license="MIT"
arch="all"
options="!check"
url="https://wezfurlong.org/wezterm/"
makedepends="cmd:tic"
source="
  target/release/wezterm
  target/release/wezterm-gui
  target/release/wezterm-mux-server
  assets/open-wezterm-here
  assets/wezterm.desktop
  assets/wezterm.appdata.xml
  assets/icon/terminal.png
  assets/icon/wezterm-icon.svg
  termwiz/data/wezterm.terminfo
"
builddir="\$srcdir"

build() {
  tic -x -o "\$builddir"/wezterm.terminfo "\$srcdir"/wezterm.terminfo
}

package() {
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/open-wezterm-here
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/wezterm
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/wezterm-gui
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/wezterm-mux-server

  install -Dm644 -t "\$pkgdir"/usr/share/applications "\$srcdir"/wezterm.desktop
  install -Dm644 -t "\$pkgdir"/usr/share/metainfo "\$srcdir"/wezterm.appdata.xml
  install -Dm644 "\$srcdir"/terminal.png "\$pkgdir"/usr/share/pixmaps/wezterm.png
  install -Dm644 "\$srcdir"/wezterm-icon.svg "\$pkgdir"/usr/share/pixmaps/wezterm.svg
  install -Dm644 "\$srcdir"/terminal.png "\$pkgdir"/usr/share/icons/hicolor/128x128/apps/wezterm.png
  install -Dm644 "\$srcdir"/wezterm-icon.svg "\$pkgdir"/usr/share/icons/hicolor/scalable/apps/wezterm.svg
  install -Dm644 "\$builddir"/wezterm.terminfo "\$pkgdir"/usr/share/terminfo/w/wezterm
}
EOF
        abuild -F checksum
        abuild -Fr
      ;;
    esac
    ;;
  *)
    ;;
esac
