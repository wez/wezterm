# `path:read_link()`

{{since('nightly')}}

Reads a symbolic link, returning a `Path` object with the path that the symlink
points to.

E.g., Dropbox is usually symlinked to `$HOME/Dropbox` on macOS, but is located
at `$HOME/Library/CloudStorage/Dropbox`.
```lua
-- macOS only
local wezterm = require 'wezterm'
local home_path = wezterm.home_path
local dropbox = home_path:join 'Dropbox'
local db_link = home_path:join 'Library/CloudStorage/Dropbox'
assert(dropbox:read_link() == db_link)
```
