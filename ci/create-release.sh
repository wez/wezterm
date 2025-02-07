#!/bin/bash
set -x
name="$1"

notes=$(cat <<EOT
See https://wezterm.org/changelog.html#$name for the changelog

If you're looking for nightly downloads or more detailed installation instructions:

[Windows](https://wezterm.org/install/windows.html)
[macOS](https://wezterm.org/install/macos.html)
[Linux](https://wezterm.org/install/linux.html)
[FreeBSD](https://wezterm.org/install/freebsd.html)
EOT
)

gh release view "$name" || gh release create --prerelease --notes "$notes" --title "$name" "$name"
