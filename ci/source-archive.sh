#!/bin/bash
# Generate a source tarball that includes git submodules

set -x

TAG_NAME=${TAG_NAME:-$(git describe --tags)}
TAG_NAME=${TAG_NAME:-$(date +'%Y%m%d-%H%M%S')-$(git log --format=%h -1)}

if [[ "$BUILD_REASON" == "Schedule" ]] ; then
  TAR_NAME=wezterm-nightly-src.tar
else
  TAR_NAME=wezterm-${TAG_NAME}-src.tar
fi

rm -f ${TAR_NAME}*

git archive --prefix=wezterm-${TAG_NAME}/ -o ${TAR_NAME} HEAD

p=`pwd`
# `git submodule foreach` outputs lines like:
# Enter 'path'
# So we need to focus on the path and strip the quotes
git submodule foreach | while read entering path; do
  path="${path%\'}";
  path="${path#\'}";
  [ "$path" = "" ] && continue;
  cd $path
  git archive --prefix=wezterm-${TAG_NAME}/$path/ HEAD > tmp.tar && \
    tar --concatenate --file=$p/${TAR_NAME} tmp.tar
  rm tmp.tar
  cd $p
done

echo $TAG_NAME > .tag
tar --owner root --group root --transform "s,^,wezterm-$TAG_NAME/," -c -f tmp.tar .tag
tar --concatenate --file=${TAR_NAME} tmp.tar
rm tmp.tar .tag

gzip ${TAR_NAME}

