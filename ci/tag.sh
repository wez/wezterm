#!/bin/bash
TAGNAME=$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)
git tag $TAGNAME

