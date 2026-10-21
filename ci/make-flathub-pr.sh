#!/usr/bin/env bash
set -xeu

if ! [[ -f Cargo.lock ]]; then
  >&2 echo "ERROR: This script should be called from repo root!"
  exit 1
fi

TAG_NAME=$(ci/tag-name.sh)

# Generate list of cargo dependencies for the isolated flatpak build
if command -v flatpak-cargo-generator &>/dev/null; then
  fcg_bin=(flatpak-cargo-generator)
else
  python3 -m pip install toml aiohttp
  curl -L 'https://github.com/flatpak/flatpak-builder-tools/raw/master/cargo/flatpak-cargo-generator.py' > /tmp/flatpak-cargo-generator.py
  fcg_bin=(python3 /tmp/flatpak-cargo-generator.py)
fi
"${fcg_bin[@]}" Cargo.lock -o assets/flatpak/generated-sources.json

URL="https://github.com/wezterm/wezterm/releases/download/${TAG_NAME}/wezterm-${TAG_NAME}-src.tar.gz"

# We require that something has obtained the source archive already and left it
# in the current dir. This is handled by actions/download-artifact in CI
SHA256=$(sha256sum wezterm*-src.tar.gz | cut -d' ' -f1)

sed -e "s,@URL@,$URL,g" -e "s/@SHA256@/$SHA256/g" < assets/flatpak/org.wezfurlong.wezterm.RELEASE-TEMPLATE.json > flathub/org.wezfurlong.wezterm.json

RELEASE_DATE=$(git -c "core.abbrev=8" show -s "--format=%cd" "--date=format:%Y-%m-%d")
sed -e "s,@TAG_NAME@,$TAG_NAME,g" -e "s/@DATE@/$RELEASE_DATE/g" < assets/flatpak/org.wezfurlong.wezterm.appdata.RELEASE-TEMPLATE.xml > flathub/org.wezfurlong.wezterm.appdata.xml

cd flathub
git config user.email wez@wezfurlong.org
git config user.name 'Wez Furlong'
git checkout -b "$TAG_NAME" origin/master
git add --all
git diff --cached
git commit -m "New version: $TAG_NAME"
git push --set-upstream origin "$TAG_NAME" --quiet
