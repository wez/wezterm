#!/bin/bash

rm -rf AppDir *.AppImage
mkdir AppDir

install -Dsm755 -t AppDir/usr/bin target/release/wezterm
install -Dm644 assets/icon/terminal.png AppDir/usr/share/icons/hicolor/128x128/apps/org.wezfurlong.wezterm.png
install -Dm644 -t AppDir/usr/share/wezterm/colors assets/colors/*
install -Dm644 assets/wezterm.desktop AppDir/usr/share/applications/org.wezfurlong.wezterm.desktop

# [appimage/stderr] /usr/bin/appstream-util: symbol lookup error: /lib64/libsoup-2.4.so.1: undefined symbol: g_file_info_get_modification_date_time
# install -Dm644 assets/wezterm.appdata.xml AppDir/usr/share/metainfo/wezterm.appdata.xml

[ -x /tmp/linuxdeploy ] || ( curl -L 'https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage' -o /tmp/linuxdeploy && chmod +x /tmp/linuxdeploy )

/tmp/linuxdeploy \
  --appdir AppDir \
  --output appimage \
  --desktop-file assets/wezterm.desktop

TAG_NAME=${TAG_NAME:-$(git describe --tags)}
TAG_NAME=${TAG_NAME:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}
distro=$(lsb_release -is)
distver=$(lsb_release -rs)

if [[ "$BUILD_REASON" == "Schedule" ]] ; then
  mv WezTerm*.AppImage WezTerm-nightly-$distro$distver.AppImage
else
  mv WezTerm*.AppImage WezTerm-$TAG_NAME-$distro$distver.AppImage
fi
