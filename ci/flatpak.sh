#!/bin/bash
set -xe

flatpak remote-add --if-not-exists --user flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --noninteractive --user flathub org.freedesktop.Platform//21.08 org.freedesktop.Sdk//21.08 org.freedesktop.Sdk.Extension.rust-stable//21.08

python3 -m pip install toml aiohttp
curl -L 'https://github.com/flatpak/flatpak-builder-tools/raw/master/cargo/flatpak-cargo-generator.py' > /tmp/flatpak-cargo-generator.py
python3 /tmp/flatpak-cargo-generator.py Cargo.lock -o assets/flatpak/generated-sources.json
flatpak-builder --install repo assets/flatpak/org.wezfurlong.wezterm.json --force-clean --user -y
