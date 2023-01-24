#!/bin/bash

tracked_markdown=$(mktemp)
trap "rm ${tracked_markdown}" "EXIT"
git ls-tree -r HEAD --name-only docs | egrep '\.(markdown|md)$' > $tracked_markdown

gelatyx --language lua --file-list $tracked_markdown --language-config ci/stylua.toml
gelatyx --language lua --file-list $tracked_markdown --language-config ci/stylua.toml --check || exit 1

set -x

# Use the GH CLI to make an authenticated request if available,
# otherwise just do an ad-hoc curl.
# However, if we are called from within a GH actions workflow (BUILD_REASON
# is set), only use `gh` if GH_TOKEN is also set, otherwise it will refuse
# to run.
function ghapi() {
  if hash gh 2>/dev/null && test \( -n "$BUILD_REASON" -a -n "$GH_TOKEN" \) -o -z "$BUILD_REASON"; then
    gh api $1
  else
    curl https://api.github.com$1
  fi
}

[[ -f /tmp/wezterm.releases.json ]] || ghapi /repos/wez/wezterm/releases > /tmp/wezterm.releases.json
[[ -f /tmp/wezterm.nightly.json ]] || ghapi /repos/wez/wezterm/releases/tags/nightly > /tmp/wezterm.nightly.json
python3 ci/subst-release-info.py || exit 1
python3 ci/generate-docs.py || exit 1
mdbook-mermaid install docs
mdbook build docs

rm gh_pages/html/README.markdown
cp assets/fonts/Symbols-Nerd-Font-Mono.ttf gh_pages/html/fonts/
cp assets/icon/terminal.png gh_pages/html/favicon.png
cp "assets/icon/wezterm-icon.svg" gh_pages/html/favicon.svg
