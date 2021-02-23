#!/bin/bash

[[ -f /tmp/wezterm.releases.json ]] || curl https://api.github.com/repos/wez/wezterm/releases > /tmp/wezterm.releases.json
python3 ci/subst-release-info.py || exit 1
python3 ci/generate-docs.py || exit 1
mdbook build docs

cp assets/icon/terminal.png gh_pages/favicon.png
cp assets/icon/wezterm-icon.svg gh_pages/favicon.svg
