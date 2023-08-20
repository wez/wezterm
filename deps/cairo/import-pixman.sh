#!/bin/bash
# Sync the vendored sources from a pixman URL
# eg:
# wget 'https://cairographics.org/releases/pixman-0.42.2.tar.gz'
# import-pixman.sh path/to/pixman-0.42.2.tar.gz
set -x
TARBALL=$1

rm -rf pixman
tar xf $TARBALL
mv pixman-* pixman
rm -rf pixman/{test,demos,configure,aclocal.m4,compile,ltmain.sh,configure.ac,config.sub,config.guess}

