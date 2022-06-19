#!/bin/bash

# Use eg: `xwininfo -int` to get the id of a wezterm
# and pass it to this script
WINID=$1

changed=$(git status --porcelain assets/colors | cut -c4-)
WEZTERM_DIR=$PWD

cd ../github/iTerm2-Color-Schemes/dynamic-colors
shots=$WEZTERM_DIR/docs/colorschemes

printf "\e]0;wezterm\e\\"

for toml in $changed ; do
  name=$(basename $toml)
  scheme=${name%.toml}.sh
  clear
  echo $scheme
  prefix=$shots/$(echo $scheme | cut -c1 | tr '[:upper:]' '[:lower:]')
  mkdir -p $prefix
  bash "./$scheme"
  bash "../tools/screenshotTable.sh"
  sleep 0.2
  xwd -id $WINID | convert "xwd:-" "png:$prefix/${scheme%.sh}.png"
done
