#!/bin/bash

[[ -f /tmp/wezterm.releases.json ]] || curl https://api.github.com/repos/wez/wezterm/releases > /tmp/wezterm.releases.json
python3 ci/subst-release-info.py
mdbook build docs

