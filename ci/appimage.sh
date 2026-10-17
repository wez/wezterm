#!/usr/bin/env bash
set -x
rm -rf AppDir *.AppImage *.zsync
set -e

mkdir AppDir

install -Dsm755 -t AppDir/usr/bin target/release/phiterm-mux-server
install -Dsm755 -t AppDir/usr/bin target/release/phiterm
install -Dsm755 -t AppDir/usr/bin target/release/phiterm-gui
install -Dsm755 -t AppDir/usr/bin target/release/strip-ansi-escapes
install -Dm644 assets/icon/terminal.png AppDir/usr/share/icons/hicolor/128x128/apps/org.phiterm.phiterm.png
install -Dm644 assets/phiterm.desktop AppDir/usr/share/applications/org.phiterm.phiterm.desktop
install -Dm644 assets/phiterm.appdata.xml AppDir/usr/share/metainfo/org.phiterm.phiterm.appdata.xml
install -Dm644 assets/phiterm-nautilus.py AppDir/usr/share/nautilus-python/extensions/phiterm-nautilus.py

[ -x /tmp/linuxdeploy ] || ( curl -L 'https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage' -o /tmp/linuxdeploy && chmod +x /tmp/linuxdeploy )

TAG_NAME=${TAG_NAME:-$(git -c "core.abbrev=8" show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S")}
distro=$(lsb_release -is 2>/dev/null || sh -c "source /etc/os-release && echo \$NAME")
distver=$(lsb_release -rs 2>/dev/null || sh -c "source /etc/os-release && echo \$VERSION_ID")

# Embed appropriate update info
# https://github.com/AppImage/AppImageSpec/blob/master/draft.md#github-releases
if [[ "$BUILD_REASON" == "Schedule" ]] ; then
  UPDATE="gh-releases-zsync|wez|phiterm|nightly|PhiTerm-*.AppImage.zsync"
  OUTPUT=PhiTerm-nightly-$distro$distver.AppImage
else
  UPDATE="gh-releases-zsync|wez|phiterm|latest|PhiTerm-*.AppImage.zsync"
  OUTPUT=PhiTerm-$TAG_NAME-$distro$distver.AppImage
fi

# Munge the path so that it finds our appstreamcli wrapper
PATH="$PWD/ci:$PATH" \
VERSION="$TAG_NAME" \
UPDATE_INFORMATION="$UPDATE" \
OUTPUT="$OUTPUT" \
  /tmp/linuxdeploy \
  --exclude-library='libwayland-client.so.0' \
  --appdir AppDir \
  --output appimage \
  --desktop-file assets/phiterm.desktop

# Update the AUR build file.  We only really want to use this for tagged
# builds but it doesn't hurt to generate it always here.
SHA256=$(sha256sum $OUTPUT | cut -d' ' -f1)
sed -e "s/@TAG@/$TAG_NAME/g" -e "s/@SHA256@/$SHA256/g" < ci/PKGBUILD.template > PKGBUILD
sed -e "s/@TAG@/$TAG_NAME/g" -e "s/@SHA256@/$SHA256/g" < ci/phiterm-linuxbrew.rb.template > phiterm-linuxbrew.rb
