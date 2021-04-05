#!/bin/bash
set -x

[[ -f /tmp/wezterm.releases.json ]] || curl https://api.github.com/repos/wez/wezterm/releases > /tmp/wezterm.releases.json
[[ -f /tmp/wezterm.nightly.json ]] || curl https://api.github.com/repos/wez/wezterm/releases/tags/nightly > /tmp/wezterm.nightly.json
python3 ci/subst-release-info.py || exit 1
python3 ci/generate-docs.py || exit 1
mdbook build docs

cp assets/icon/terminal.png gh_pages/html/favicon.png
cp assets/icon/wezterm-icon.svg gh_pages/html/favicon.svg
