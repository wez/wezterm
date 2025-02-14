# `path:file_stem()`

{{since('nightly')}}

Returns a string containing the non-extension portion of `path:basename()`,
i.e., the non-extension portion of the filename.

If there is no filename, it returns the empty string. If the filename has
no embedded `.` (after the first character), then it returns the entire filename.
Otherwise it returns the portion of the filename before the final `.`.

```lua
local wezterm = require 'wezterm'
local to_path = wezterm.to_path

local txt = to_path '/file/path.txt'
assert(txt.file_stem() == 'path')
local tar_gz = to_path '/file/path.tar.gz'
assert(tar_gz.file_stem() == 'path.tar')
local path = to_path '/file/path'
assert(path.file_stem() == 'path')
```

See also [extension](extension.md).
