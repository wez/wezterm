#!/bin/sh
cd $(git rev-parse --show-toplevel)
docker run -t \
  -v "$(pwd)":/app/ \
  --workdir /app/termwiz \
  "ghcr.io/orhun/git-cliff/git-cliff:latest" \
  --tag-pattern 'termwiz-*' -o CHANGELOG.md -c cliff.toml
