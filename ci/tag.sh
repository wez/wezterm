#!/usr/bin/env bash
TAGNAME=$(./ci/tag-name.sh)
git tag $TAGNAME

