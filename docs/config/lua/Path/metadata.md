# `path:metadata()`

{{since('nightly')}}

Queries the file system to get information about a directory, file or symlink,
and returns a [`MetaData`](../MetaData/index.md) object.

This function will traverse symbolic links to query information about the destination
file.

