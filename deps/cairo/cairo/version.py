#!/usr/bin/env python3
#
# cairo version.py
#
# Extracts the version from cairo-version.h for the meson build files.
#
import os
import re
import sys


MAJOR_RE = re.compile(
    r'^\s*#\s*define\s+CAIRO_VERSION_MAJOR\s+(?P<number>[0-9]+)\s*$',
    re.UNICODE)

MINOR_RE = re.compile(
    r'^\s*#\s*define\s+CAIRO_VERSION_MINOR\s+(?P<number>[0-9]+)\s*$',
    re.UNICODE)

MICRO_RE = re.compile(
    r'^\s*#\s*define\s+CAIRO_VERSION_MICRO\s+(?P<number>[0-9]+)\s*$',
    re.UNICODE)

version_major = None
version_minor = None
version_micro = None

srcroot = os.path.dirname(__file__)
version_h = os.path.join(srcroot, "src", "cairo-version.h")

with open(version_h, "r", encoding="utf-8") as f:
    for line in f:
        res = MAJOR_RE.match(line)
        if res:
            assert version_major is None
            version_major = res.group('number')
            continue
        res = MINOR_RE.match(line)
        if res:
            assert version_minor is None
            version_minor = res.group('number')
            continue
        res = MICRO_RE.match(line)
        if res:
            assert version_micro is None
            version_micro = res.group('number')
            continue

if not (version_major and version_minor and version_micro):
    print(f"ERROR: Could not extract version from cairo-version.h in {srcroot}", file=sys.stderr)  # noqa
    sys.exit(-1)

print(f"{version_major}.{version_minor}.{version_micro}")
