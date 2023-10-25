# `meta:is_symlink()`

{{since('nightly')}}

Returns `true` if `meta` is the `MetaData` of a symlink and `false` otherwise.

*Note:* This method will only work as expected if you have gotten the `MetaData`
object `meta` via [`symlink_metadata`](../Path/symlink_metadata.md).
