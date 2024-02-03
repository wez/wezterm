#!/usr/bin/env python3
import glob
import os
import re
import sys

NIGHTLY = '20240203-110809-5046fc22'

SINCE = re.compile("\{\{since\('nightly'", re.MULTILINE)

for p in ["docs/**/*.md", "docs/**/*.markdown"]:
    for filename in glob.glob(p, recursive=True):
        with open(filename, "r") as f:
            content = f.read()

        adjusted = SINCE.sub(f"{{{{since('{NIGHTLY}'", content)
        if content != adjusted:
            print(filename)
            with open(filename, "w") as f:
                f.truncate()
                f.write(adjusted)

SCHEME_DATA = 'docs/colorschemes/data.json'
with open(SCHEME_DATA, 'r') as f:
    content = f.read()
with open(SCHEME_DATA, 'w') as f:
    content = content.replace("nightly builds only", NIGHTLY)
    f.truncate()
    f.write(content)
