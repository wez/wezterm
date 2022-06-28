#!/bin/bash
set -x
name="$1"
gh release view "$name" || gh release create --prerelease --notes "See https://wezfurlong.org/wezterm/changelog.html#$name for the changelog" --title "$name" "$name"
