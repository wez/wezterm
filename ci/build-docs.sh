#!/bin/bash

[[ -f /tmp/wezterm.releases.json ]] || curl https://api.github.com/repos/wez/wezterm/releases > /tmp/wezterm.releases.json
python3 ci/subst-release-info.py
python3 ci/generate-docs.py
mdbook build docs

# mdBook can append js includes but it is too late to register syntax
# highlighting extensions, so we apply brute force here

mv gh_pages/book.js gh_pages/book.2
cat docs/lua.js gh_pages/book.2 > gh_pages/book.js
rm gh_pages/book.2
cp assets/icon/terminal.png gh_pages/favicon.png
cp assets/icon/wezterm-icon.svg gh_pages/favicon.svg
