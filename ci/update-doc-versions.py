#!/usr/bin/env python3
import glob
import os
import re
import sys

NIGHTLY = '20230326-111934-3666303c'

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
