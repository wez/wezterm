## quote_dropped_files = "SpacesOnly"

*Since: nightly builds only*

Controls how file names are quoted (or not) when dragging and dropping.
There are three possible values:

* `"None"` - no quoting is performed, the file name is passed through as-is.
* `"SpacesOnly"` - backslash-escape only spaces, leaving all other characters as-is.  This is the default.
* `"Posix"` - use POSIX style shell word escaping.

For example:

|`quote_dropped_files` |file name    |quoted result  |
|----------------------|-------------|---------------|
|`"None"`              |`hello world`|`hello world`  |
|`"SpacesOnly"`        |`hello world`|`hello\ world` |
|`"Posix"`             |`hello world`|`"hello world"`|

Files drag and drop support is a platform dependent feature

|Platform  |Supported since    |
|----------|-------------------|
|macOS     |nightly builds only|
|Windows   |Not yet            |
|X11       |Not yet            |
|Wayland   |Not yet            |
