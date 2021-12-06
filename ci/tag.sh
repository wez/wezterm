#!/bin/bash
TAGNAME=$(git -c "core.abbrev=8" show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S")
git tag $TAGNAME

