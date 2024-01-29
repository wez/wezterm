# Url object

{{since('20240127-113634-bbcac864')}}

The `Url` object represents a parsed Url.  It has the following fields:

* `scheme` - the URL scheme such as `"file"`, or `"https"`
* `file_path` - decodes the `path` field and interprets it as a file path
* `username` - the username portion of the URL, or an empty string if none is specified
* `password` - the password portion of the URL, or `nil` if none is specified
* `host` - the hostname portion of the URL, with IDNA decoded to UTF-8
* `path` - the path portion of the URL, complete with percent encoding
* `fragment` - the fragment portion of the URL
* `query` - the query portion of the URL

```lua
local wezterm = require 'wezterm'

local url = wezterm.url.parse 'file://myhost/some/path%20with%20spaces'
assert(url.scheme == 'file')
assert(url.file_path == '/some/path with spaces')

local url =
  wezterm.url.parse 'https://github.com/rust-lang/rust/issues?labels=E-easy&state=open'
assert(url.scheme == 'https')
assert(url.username == '')
assert(url.password == nil)
assert(url.host == 'github.com')
assert(url.path == '/rust-lang/rust/issues')
assert(url.query == 'labels=E-easy&state=open')
```

