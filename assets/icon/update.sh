#!/bin/bash
# This script updates the icon files from the svg file.
# It assumes that the svg file is square.
set -x
cd $(git rev-parse --show-toplevel)/assets/icon

src=wezterm-icon.svg

conv_opts="-colors 256 -background none -density 300"

# the linux icon
convert $conv_opts -resize "!128x128" "$src" ../icon/terminal.png

for dim in 16 32 128 256 512 1024 ; do
  # convert is the imagemagick convert utility
  convert $conv_opts -border '10%' -bordercolor 'rgba(0,0,0,0)' -resize "!${dim}x${dim}" "$src" "icon_${dim}px.png"
done
# png2icns is part of the libicns-utils on Fedora systems.
# It glues together the various png files into a macOS .icns file
png2icns ../macos/WezTerm.app/Contents/Resources/terminal.icns icon_*px.png

# Clean up
rm -f icon_*px.png

# The Windows icon
convert $conv_opts -define icon:auto-resize=256,128,96,64,48,32,16 $src ../windows/terminal.ico

