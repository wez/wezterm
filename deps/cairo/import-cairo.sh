#!/bin/bash
# Sync the vendored sources from a pixman URL
# eg:
# wget 'https://cairographics.org/snapshots/cairo-1.17.8.tar.xz'
# import-cairo.sh path/to/cairo-1.17.8.tar.xz
set -x
TARBALL=$1

rm -rf cairo
tar xf $TARBALL
mv cairo-* cairo
rm -rf cairo/{test,perf,doc,meson-cc-tests,boilerplate,subprojects,.git*}
rm -rf cairo/util/{cairo-fdr,cairo-gobject,cairo-script,cairo-sphinx,cairo-trace,show-*.c}
rm -rf cairo/src/{*.awk,*sh,win32,*-xlib-*,*-xcb-*,.git*}
cat > cairo/src/config.h <<-EOT
#pragma once
EOT

cat > cairo/src/cairo-features.h <<-EOT
#pragma once
#define CAIRO_FEATURES_H
#define CAIRO_HAS_IMAGE_SURFACE 1
#define CAIRO_HAS_MIME_SURFACE 1
#define CAIRO_HAS_OBSERVER_SURFACE 1
#define CAIRO_HAS_RECORDING_SURFACE 1
#define CAIRO_HAS_TEE_SURFACE 1
#define CAIRO_HAS_USER_FONT 1
EOT
