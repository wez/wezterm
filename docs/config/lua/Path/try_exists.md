# `path:try_exists()`

{{since('nightly')}}

Returns `true` if `path` points at an existing directory, file or (working)
symlink, and returns `false` otherwise. This will traverse symbolic links
to query information about the destination directory or file. In case of a
broken symlink it will return `false`.

If the existence of the directory or file pointed to by `path` can neither
be confirmed or denied, then this function will error. This can happen e.g.,
if permission to access the path is denied.

```lua
local wezterm = require 'wezterm'
local home_path = wezterm.home_path
assert(home_path:try_exists())
```
