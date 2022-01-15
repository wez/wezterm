#!/bin/bash
TAGNAME=$(./ci/tag-name.sh)
git tag $TAGNAME

