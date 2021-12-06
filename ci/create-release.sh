#!/bin/bash
set -x
name="$1"
gh release view "$name" || gh release create --prerelease --draft "$name"
