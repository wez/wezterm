#!/usr/bin/env bash
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

echo "OSTYPE is $OSTYPE"

case $OSTYPE in
  darwin*)
    zipdir=PhiTerm-macos-$TAG_NAME
    if [[ "$BUILD_REASON" == "Schedule" ]] ; then
      zipname=PhiTerm-macos-nightly.zip
    else
      zipname=$zipdir.zip
    fi
    rm -rf $zipdir $zipname
    mkdir $zipdir
    cp -r assets/macos/Phiterm.app $zipdir/
    # Omit MetalANGLE for now; it's a bit laggy compared to CGL,
    # and on M1/Big Sur, CGL is implemented in terms of Metal anyway
    rm $zipdir/Phiterm.app/*.dylib
    mkdir -p $zipdir/Phiterm.app/Contents/MacOS
    mkdir -p $zipdir/Phiterm.app/Contents/Resources
    cp -r assets/shell-integration/* $zipdir/Phiterm.app/Contents/Resources
    cp -r assets/shell-completion $zipdir/Phiterm.app/Contents/Resources
    tic -xe wezterm -o $zipdir/Phiterm.app/Contents/Resources/terminfo termwiz/data/wezterm.terminfo

    for bin in phiterm phiterm-mux-server phiterm-gui strip-ansi-escapes ; do
      # If the user ran a simple `cargo build --release`, then we want to allow
      # a single-arch package to be built
      if [[ -f $TARGET_DIR/release/$bin ]] ; then
        cp $TARGET_DIR/release/$bin $zipdir/Phiterm.app/Contents/MacOS/$bin
      else
        # The CI runs `cargo build --target XXX --release` which means that
        # the binaries will be deployed in `$TARGET_DIR/XXX/release` instead of
        # the plain path above.
        # In that situation, we have two architectures to assemble into a
        # Universal ("fat") binary, so we use the `lipo` tool for that.
        lipo $TARGET_DIR/*/release/$bin -output $zipdir/Phiterm.app/Contents/MacOS/$bin -create
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
        --entitlements ci/macos-entitlement.plist --deep --sign "$MACOS_TEAM_ID" $zipdir/Phiterm.app/
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
    sed -e "s/@TAG@/$TAG_NAME/g" -e "s/@SHA256@/$SHA256/g" < ci/phiterm-homebrew-macos.rb.template > phiterm.rb

    ;;
  msys|cygwin)
    zipdir=PhiTerm-windows-$TAG_NAME
    if [[ "$BUILD_REASON" == "Schedule" ]] ; then
      zipname=PhiTerm-windows-nightly.zip
      instname=PhiTerm-nightly-setup
    else
      zipname=$zipdir.zip
      instname=PhiTerm-${TAG_NAME}-setup
    fi
    rm -rf $zipdir $zipname
    mkdir $zipdir
    cp $TARGET_DIR/release/phiterm.exe \
      $TARGET_DIR/release/phiterm-mux-server.exe \
      $TARGET_DIR/release/phiterm-gui.exe \
      $TARGET_DIR/release/strip-ansi-escapes.exe \
      $TARGET_DIR/release/phiterm.pdb \
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
        PHITERM_RPM_VERSION=$(echo ${TAG_NAME#nightly-} | tr - _)
        distroid=$(sh -c "source /etc/os-release && echo \$ID" | tr - _)
        distver=$(sh -c "source /etc/os-release && echo \$VERSION_ID" | tr - _)

        SPEC_RELEASE="1.${distroid}${distver}"
        if test -n "${COPR_SRPM}" ; then
          SPEC_RELEASE=0
        fi

        # Set up variables for spec generation
        if test -n "${COPR_SRPM}" ; then
          TAR_NAME=$(git -c "core.abbrev=8" show -s "--format=%cd_%h" "--date=format:%Y%m%d_%H%M%S")
          HERE="."
          BUILD_SECTION=$(cat <<'BUILDEOFEOF'
%prep
%autosetup
%build

echo Here I am

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

cargo build --release \
      -p phiterm-gui -p phiterm -p phiterm-mux-server \
      -p strip-ansi-escapes
BUILDEOFEOF
)
          BUILD_REQUIRES=$(cat <<BREQEOF
BuildRequires: gcc, gcc-c++, make, curl, fontconfig-devel, openssl-devel, libxcb-devel, libxkbcommon-devel, libxkbcommon-x11-devel, wayland-devel, xcb-util-devel, xcb-util-keysyms-devel, xcb-util-image-devel, xcb-util-wm-devel, git
%if 0%{?suse_version}
BuildRequires: Mesa-libEGL-devel
%else
BuildRequires: mesa-libEGL-devel
%endif
%if 0%{?fedora} >= 41 && 0%{?fedora} < 45
BuildRequires: openssl-devel-engine
%endif
Source0: phiterm-${TAR_NAME}.tar.gz
BREQEOF
)
        else
          HERE="${HERE}"
          BUILD_SECTION=$(cat <<'BUILDEOFEOF'
%build
echo build
BUILDEOFEOF
)
          BUILD_REQUIRES=""
        fi

        # Generate single spec with subpackages
        cat > phiterm.spec <<EOF
Name: phiterm
Version: ${PHITERM_RPM_VERSION}
Release: ${SPEC_RELEASE}
Packager: Wez Furlong <wez@wezfurlong.org>
License: MIT
URL: https://wezterm.org/
Summary: PhiTerm.
${BUILD_REQUIRES}
Requires: phiterm-common, phiterm-gui, phiterm-mux-server

%global debug_package %{nil}

%description
phiterm is a terminal emulator with support for modern features
such as fonts with ligatures, hyperlinks, tabs and multiple
windows.

# Subpackage: phiterm-common
%package -n phiterm-common
Summary: PhiTerm - Common CLI components
Requires: openssl
%description -n phiterm-common
phiterm-common provides the base CLI launcher and utilities shared by
all phiterm components.

# Subpackage: phiterm-gui
%package -n phiterm-gui
Summary: PhiTerm - GUI and multiplexer
Requires: phiterm-common
%if 0%{?suse_version}
Requires: dbus-1, fontconfig, libxcb1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libwayland-egl1, libwayland-cursor0, Mesa-libEGL1, libxcb-keysyms1, libxcb-ewmh2, libxcb-icccm4
%else
Requires: dbus, fontconfig, libxcb, libxkbcommon, libxkbcommon-x11, libwayland-client, libwayland-egl, libwayland-cursor, mesa-libEGL, xcb-util-keysyms, xcb-util-wm
%endif
%description -n phiterm-gui
phiterm-gui is a GPU-accelerated cross-platform terminal emulator with
support for modern features such as fonts with ligatures, hyperlinks,
tabs and multiple windows.

# Subpackage: phiterm-mux-server
%package -n phiterm-mux-server
Summary: PhiTerm - Multiplexer server (headless)
Requires: openssl
%description -n phiterm-mux-server
phiterm-mux-server is a headless terminal multiplexer that can be used
as a session manager for terminal sessions, without requiring X11,
Wayland, or other GUI libraries.

${BUILD_SECTION}

%install
set -x
cd ${HERE}
mkdir -p %{buildroot}/usr/bin %{buildroot}/etc/profile.d %{buildroot}/usr/share/icons/hicolor/128x128/apps %{buildroot}/usr/share/applications %{buildroot}/usr/share/metainfo %{buildroot}/usr/share/nautilus-python/extensions
install -Dm755 assets/open-phiterm-here -t %{buildroot}/usr/bin
install -Dsm755 $TARGET_DIR/release/phiterm -t %{buildroot}/usr/bin
install -Dsm755 $TARGET_DIR/release/phiterm-gui -t %{buildroot}/usr/bin
install -Dsm755 $TARGET_DIR/release/phiterm-mux-server -t %{buildroot}/usr/bin
install -Dsm755 $TARGET_DIR/release/strip-ansi-escapes -t %{buildroot}/usr/bin
install -Dm644 assets/shell-integration/* -t %{buildroot}/etc/profile.d
install -Dm644 assets/shell-completion/zsh %{buildroot}/usr/share/zsh/site-functions/_phiterm
install -Dm644 assets/shell-completion/bash %{buildroot}/etc/bash_completion.d/phiterm
install -Dm644 assets/icon/terminal.png %{buildroot}/usr/share/icons/hicolor/128x128/apps/org.phiterm.phiterm.png
install -Dm644 assets/phiterm.desktop %{buildroot}/usr/share/applications/org.phiterm.phiterm.desktop
install -Dm644 assets/phiterm.appdata.xml %{buildroot}/usr/share/metainfo/org.phiterm.phiterm.appdata.xml
install -Dm644 assets/phiterm-nautilus.py %{buildroot}/usr/share/nautilus-python/extensions/phiterm-nautilus.py

%files
# Main package (metapackage) has no files

%files -n phiterm-common
/usr/bin/phiterm
/usr/bin/strip-ansi-escapes
/usr/share/zsh/site-functions/_phiterm
/etc/bash_completion.d/phiterm
/etc/profile.d/*

%files -n phiterm-gui
/usr/bin/open-phiterm-here
/usr/bin/phiterm-gui
/usr/share/icons/hicolor/128x128/apps/org.phiterm.phiterm.png
/usr/share/applications/org.phiterm.phiterm.desktop
/usr/share/metainfo/org.phiterm.phiterm.appdata.xml
/usr/share/nautilus-python/extensions/phiterm-nautilus.py*

%files -n phiterm-mux-server
/usr/bin/phiterm-mux-server

%changelog
* Mon Oct 2 2023 Wez Furlong
- See git for full changelog
EOF

        if test -n "${COPR_SRPM}" ; then
          /usr/bin/rpmbuild -bs --rmspec phiterm.spec --verbose
          mv $(rpm --eval '%{_srcrpmdir}')/phiterm-${TAR_NAME}*.src.rpm "${COPR_SRPM}"/
        else
          /usr/bin/rpmbuild -bb --rmspec phiterm.spec --verbose
        fi

        ;;
      Ubuntu*|Debian*|Pop)
        rm -rf pkg
        mkdir -p pkg/debian/usr/bin pkg/debian/DEBIAN pkg/debian/usr/share/{applications,phiterm}

        if [[ "$BUILD_REASON" == "Schedule" ]] ; then
          pkgname=phiterm-nightly
          conflicts=phiterm
        else
          pkgname=phiterm
          conflicts=phiterm-nightly
        fi

        cat > pkg/debian/control <<EOF
Package: $pkgname
Version: ${TAG_NAME#nightly-}
Conflicts: $conflicts
Architecture: $(dpkg-architecture -q DEB_BUILD_ARCH_CPU)
Maintainer: Wez Furlong <wez@wezfurlong.org>
Section: utils
Priority: optional
Homepage: https://wezterm.org/
Description: PhiTerm.
 phiterm is a terminal emulator with support for modern features
 such as fonts with ligatures, hyperlinks, tabs and multiple
 windows.
Provides: x-terminal-emulator
Source: https://wezterm.org/
EOF

        cat > pkg/debian/postinst <<EOF
#!/bin/sh
set -e
if [ "\$1" = "configure" ] ; then
        update-alternatives --install /usr/bin/x-terminal-emulator x-terminal-emulator /usr/bin/open-phiterm-here 20
fi
EOF

        cat > pkg/debian/prerm <<EOF
#!/bin/sh
set -e
if [ "\$1" = "remove" ]; then
	update-alternatives --remove x-terminal-emulator /usr/bin/open-phiterm-here
fi
EOF

        install -Dsm755 -t pkg/debian/usr/bin $TARGET_DIR/release/phiterm-mux-server
        install -Dsm755 -t pkg/debian/usr/bin $TARGET_DIR/release/phiterm-gui
        install -Dsm755 -t pkg/debian/usr/bin $TARGET_DIR/release/phiterm
        install -Dm755 -t pkg/debian/usr/bin assets/open-phiterm-here
        install -Dsm755 -t pkg/debian/usr/bin $TARGET_DIR/release/strip-ansi-escapes

        deps=$(cd pkg && dpkg-shlibdeps -O -e debian/usr/bin/*)
        mv pkg/debian/postinst pkg/debian/DEBIAN/postinst
        chmod 0755 pkg/debian/DEBIAN/postinst
        mv pkg/debian/prerm pkg/debian/DEBIAN/prerm
        chmod 0755 pkg/debian/DEBIAN/prerm
        mv pkg/debian/control pkg/debian/DEBIAN/control
        sed -i '/^Source:/d' pkg/debian/DEBIAN/control  # The `Source:` field needs to be valid in a binary package
        echo $deps | sed -e 's/shlibs:Depends=/Depends: /' >> pkg/debian/DEBIAN/control
        cat pkg/debian/DEBIAN/control

        install -Dm644 assets/icon/terminal.png pkg/debian/usr/share/icons/hicolor/128x128/apps/org.phiterm.phiterm.png
        install -Dm644 assets/phiterm.desktop pkg/debian/usr/share/applications/org.phiterm.phiterm.desktop
        install -Dm644 assets/phiterm.appdata.xml pkg/debian/usr/share/metainfo/org.phiterm.phiterm.appdata.xml
        install -Dm644 assets/phiterm-nautilus.py pkg/debian/usr/share/nautilus-python/extensions/phiterm-nautilus.py
        install -Dm644 assets/shell-completion/bash pkg/debian/usr/share/bash-completion/completions/phiterm
        install -Dm644 assets/shell-completion/zsh pkg/debian/usr/share/zsh/functions/Completion/Unix/_phiterm
        install -Dm644 assets/shell-integration/* -t pkg/debian/etc/profile.d

        if [[ "$BUILD_REASON" == "Schedule" ]] ; then
          debname=phiterm-nightly.$distro$distver
        else
          debname=phiterm-$TAG_NAME.$distro$distver
        fi
        arch=$(dpkg-architecture -q DEB_BUILD_ARCH_CPU)
        case $arch in
          amd64)
            ;;
          *)
            debname="${debname}.${arch}"
            ;;
        esac

        fakeroot dpkg-deb --build pkg/debian $debname.deb

        if [[ "$BUILD_REASON" != '' ]] ; then
          $SUDO apt-get install ./$debname.deb
        fi

        mv pkg/debian pkg/phiterm
        tar cJf $debname.tar.xz -C pkg phiterm
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
pkgname=phiterm
pkgver=$(echo "$pkgver" | cut -d'-' -f1-2 | tr - .)
_pkgver=$pkgver
pkgrel=0
pkgdesc="A GPU-accelerated cross-platform terminal emulator and multiplexer written in Rust"
license="MIT"
arch="all"
options="!check"
url="https://wezterm.org/"
makedepends="cmd:tic"
source="
  $TARGET_DIR/release/phiterm
  $TARGET_DIR/release/phiterm-gui
  $TARGET_DIR/release/phiterm-mux-server
  assets/open-phiterm-here
  assets/phiterm.desktop
  assets/phiterm.appdata.xml
  assets/icon/terminal.png
  assets/icon/phiterm-icon.svg
  termwiz/data/wezterm.terminfo
"
builddir="\$srcdir"

build() {
  tic -x -o "\$builddir"/wezterm.terminfo "\$srcdir"/wezterm.terminfo
}

package() {
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/open-phiterm-here
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/phiterm
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/phiterm-gui
  install -Dm755 -t "\$pkgdir"/usr/bin "\$srcdir"/phiterm-mux-server

  install -Dm644 -t "\$pkgdir"/usr/share/applications "\$srcdir"/phiterm.desktop
  install -Dm644 -t "\$pkgdir"/usr/share/metainfo "\$srcdir"/phiterm.appdata.xml
  install -Dm644 "\$srcdir"/terminal.png "\$pkgdir"/usr/share/pixmaps/phiterm.png
  install -Dm644 "\$srcdir"/phiterm-icon.svg "\$pkgdir"/usr/share/pixmaps/phiterm.svg
  install -Dm644 "\$srcdir"/terminal.png "\$pkgdir"/usr/share/icons/hicolor/128x128/apps/phiterm.png
  install -Dm644 "\$srcdir"/phiterm-icon.svg "\$pkgdir"/usr/share/icons/hicolor/scalable/apps/phiterm.svg
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
