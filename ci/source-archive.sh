#!/bin/bash
# Generate a source tarball that includes git submodules

set -x

TAG_NAME=${TAG_NAME:-$(git -c "core.abbrev=8" show -s "--format=%cd-%h" "--date=format:%Y%m%d-%H%M%S")}

if [[ "$BUILD_REASON" == "Schedule" ]] ; then
  TAR_NAME=wezterm-nightly-src.tar
else
  TAR_NAME=wezterm-${TAG_NAME}-src.tar
fi

if test -n "${COPR_SRPM}" ; then
  TAG_NAME=$(git -c "core.abbrev=8" show -s "--format=%cd_%h" "--date=format:%Y%m%d_%H%M%S")
  TAR_NAME=wezterm-${TAG_NAME}.tar
fi

rm -f ${TAR_NAME}*

NAME_PREFIX=wezterm-${TAG_NAME}

git archive --prefix=${NAME_PREFIX}/ -o ${TAR_NAME} HEAD

p=`pwd`
# `git submodule foreach` outputs lines like:
# Enter 'path'
# So we need to focus on the path and strip the quotes
git submodule foreach | while read entering path; do
  path="${path%\'}";
  path="${path#\'}";
  [ "$path" = "" ] && continue;
  cd $path
  git archive --prefix=${NAME_PREFIX}/$path/ HEAD > tmp.tar && \
    tar --concatenate --file=$p/${TAR_NAME} tmp.tar
  rm tmp.tar
  cd $p
done

echo $TAG_NAME > .tag
tar --owner root --group root --transform "s,^,$NAME_PREFIX/," -c -f tmp.tar .tag
tar --concatenate --file=${TAR_NAME} tmp.tar
rm tmp.tar .tag

# Remove bulky bits that are not required to build from source; this helps
# to keep the source tarball small!
tar --delete \
  ${NAME_PREFIX}/deps/harfbuzz/harfbuzz/test \
  ${NAME_PREFIX}/deps/freetype/libpng/contrib \
  ${NAME_PREFIX}/docs/screenshots \
  -f ${TAR_NAME}

gzip ${TAR_NAME}

if test -n "${COPR_SRPM}" ; then
  mv ${TAR_NAME}.gz $(rpm --eval '%{_sourcedir}')
fi
