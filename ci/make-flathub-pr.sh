#!/bin/bash
set -xe
TAG_NAME=$(ci/tag-name.sh)

TAG_NAME=20230320-124340-559cb7b0 # FIXME for remove this line

URL="https://github.com/wez/wezterm/releases/download/${TAG_NAME}/wezterm-${TAG_NAME}-src.tar.gz"

curl -L "$URL" -o tarball
SHA256=$(sha256sum tarball | cut -d' ' -f1)

sed -e "s,@URL@,$URL,g" -e "s/@SHA256@/$SHA256/g" < assets/flatpak/org.wezfurlong.wezterm.template.json > flathub/org.wezfurlong.wezterm.json

TAG_NAME=$(ci/tag-name.sh)   # FIXME: remove this line

RELEASE_DATE=$(git -c "core.abbrev=8" show -s "--format=%cd" "--date=format:%Y-%m-%d")
sed -e "s,@TAG_NAME@,$TAG_NAME,g" -e "s/@DATE@/$RELEASE_DATE/g" < assets/flatpak/org.wezfurlong.wezterm.appdata.template.xml > flathub/org.wezfurlong.wezterm.appdata.xml

cd flathub
git checkout -b "$TAG_NAME" origin/master

git add --all
git diff --cached
git commit -m "New version: $TAG_NAME"
git push --set-upstream origin "$TAG_NAME" --quiet
