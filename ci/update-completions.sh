#!/bin/bash

for shell in bash zsh fish ; do
  target/debug/wezterm shell-completion --shell $shell > assets/shell-completion/$shell
done
