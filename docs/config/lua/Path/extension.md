# `path:extension()`

{{since('nightly')}}

Returns a string containing the extension portion of `path:basename()`,
i.e., the extension portion of the filename.

If there is no filename, no embedded `.` (after the first character), it returns
the empty string. Otherwise it returns the portion of the filename after the final
`.`.

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

See also [file_stem](file_stem.md).
