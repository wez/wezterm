#!/bin/bash
set -x
name="$1"

notes=$(cat <<EOT
See https://wezfurlong.org/wezterm/changelog.html#$name for the changelog

If you're looking for nightly downloads or more detailed installation instructions:

[Windows](https://wezfurlong.org/wezterm/install/windows.html)
[macOS](https://wezfurlong.org/wezterm/install/macos.html)
[Linux](https://wezfurlong.org/wezterm/install/linux.html)
[FreeBSD](https://wezfurlong.org/wezterm/install/freebsd.html)
EOT
)

gh release view "$name" || gh release create --prerelease --notes "$notes" --title "$name" "$name"
