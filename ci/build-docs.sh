#!/bin/bash

SERVE=no
if [ "$1" == "serve" ] ; then
  SERVE=yes
fi

for util in gelatyx ; do
  if ! hash $util 2>/dev/null ; then
    cargo install $util --locked
  fi
done

tracked_markdown=$(mktemp)
trap "rm ${tracked_markdown}" "EXIT"
find docs -type f | egrep '\.(markdown|md)$' > $tracked_markdown

gelatyx --language lua --file-list $tracked_markdown --language-config ci/stylua.toml
gelatyx --language lua --file-list $tracked_markdown --language-config ci/stylua.toml --check || exit 1

set -ex

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

# Adjust path to pick up pip-installed binaries
PATH="$HOME/.local/bin;$PATH"

PIP=pip3
if ! hash pip3 >/dev/null ; then
  PIP=pip
fi

$PIP install --quiet mkdocs-material mkdocs-git-revision-date-localized-plugin black mkdocs-exclude mkdocs-include-markdown-plugin mkdocs-macros-plugin
if test -n "${CARDS}" ; then
  $PIP install --quiet pillow cairosvg
fi

black ci/generate-docs.py ci/subst-release-info.py

cp "assets/icon/terminal.png" docs/favicon.png
cp "assets/icon/wezterm-icon.svg" docs/favicon.svg
mkdir -p docs/fonts
cp assets/fonts/SymbolsNerdFontMono-Regular.ttf docs/fonts/

if [ "$SERVE" == "yes" ] ; then
  mkdocs "$@"
else
  mkdocs build
fi
