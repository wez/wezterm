#!/usr/bin/env python3
import glob
import os
import re
import sys

NIGHTLY = '20230320-124340-559cb7b0'

NIGHTLY_UPDATE = re.compile('[Ss]ince: nightly builds only', re.MULTILINE)
OLD_SINCE = re.compile('^\*[Ss]ince: (\\S+)\*', re.MULTILINE)
OLD_SINCE_INLINE = re.compile('\(?\*?[Ss]ince: (\\S+)\*?\)?', re.MULTILINE)

for p in ["docs/**/*.md", "docs/**/*.markdown"]:
    for filename in glob.glob(p, recursive=True):
        with open(filename, "r") as f:
            content = f.read()

        adjusted = NIGHTLY_UPDATE.sub(f"Since: {NIGHTLY}", content)
        adjusted = OLD_SINCE.sub("{{since('\\1')}}", adjusted)
        adjusted = OLD_SINCE_INLINE.sub("{{since('\\1', inline=True)}}", adjusted)
        if content != adjusted:
            print(filename)
            with open(filename, "w") as f:
                f.truncate()
                f.write(adjusted)
