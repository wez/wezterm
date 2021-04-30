#!/bin/bash
TAGNAME=$(git show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S")
git tag $TAGNAME

