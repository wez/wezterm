#!/bin/bash

tracked_markdown=$(git ls-tree -r HEAD --name-only docs | egrep '\.(markdown|md)$')

gelatyx lua --file $tracked_markdown --language-config ci/stylua.toml
gelatyx lua --file $tracked_markdown --language-config ci/stylua.toml --check || exit 1

set -x

[[ -f /tmp/wezterm.releases.json ]] || curl https://api.github.com/repos/wez/wezterm/releases > /tmp/wezterm.releases.json
[[ -f /tmp/wezterm.nightly.json ]] || curl https://api.github.com/repos/wez/wezterm/releases/tags/nightly > /tmp/wezterm.nightly.json
python3 ci/subst-release-info.py || exit 1
python3 ci/generate-docs.py || exit 1
mdbook-mermaid install docs
mdbook build docs

rm gh_pages/html/README.markdown
cp assets/fonts/Symbols-Nerd-Font-Mono.ttf gh_pages/html/fonts/
cp assets/icon/terminal.png gh_pages/html/favicon.png
cp "assets/icon/wezterm-icon.svg" gh_pages/html/favicon.svg
