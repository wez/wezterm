#!/usr/bin/env bash
set -xeu

if ! [[ -f Cargo.lock ]]; then
  >&2 echo "ERROR: This script should be called from repo root!"
  exit 1
fi

# Add flathub repo if not available
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo

# Install SDK & Platform flatpak components
# NOTE: We install specific version of those components, make sure it corresponds with the
# runtime-version used in ../assets/flatpak/org.wezfurlong.wezterm.LOCAL-TESTING.json
flatpak install --user --noninteractive flathub \
  org.freedesktop.Platform//25.08 \
  org.freedesktop.Sdk//25.08 \
  org.freedesktop.Sdk.Extension.rust-stable//25.08

# Disabled for now: seems like it has an OpenSSL problem and fails to use SSL when
# validating the screenshot URLs
#flatpak install --user --noninteractive org.freedesktop.appstream-glib
#flatpak run --env=G_DEBUG=fatal-criticals org.freedesktop.appstream-glib validate assets/wezterm.appdata.xml

# Generate list of cargo dependencies for the isolated flatpak build
if command -v flatpak-cargo-generator &>/dev/null; then
  fcg_bin=(flatpak-cargo-generator)
else
  python3 -m pip install toml aiohttp
  curl -L 'https://github.com/flatpak/flatpak-builder-tools/raw/master/cargo/flatpak-cargo-generator.py' > /tmp/flatpak-cargo-generator.py
  fcg_bin=(python3 /tmp/flatpak-cargo-generator.py)
fi
"${fcg_bin[@]}" Cargo.lock -o assets/flatpak/generated-sources.json

# Build & install the flatpak locally
# NOTE: Disabled in CI
if [ "${CI:-}" != "yes" ] ; then
  flatpak-builder \
    --state-dir /var/tmp/wezterm-flatpak-builder \
    --install /var/tmp/wezterm-flatpak-repo \
    assets/flatpak/org.wezfurlong.wezterm.LOCAL-TESTING.json \
    --user --force-clean -y
fi
