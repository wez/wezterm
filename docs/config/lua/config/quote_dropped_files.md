## quote_dropped_files = "SpacesOnly"

*Since: nightly builds only*

Controls how file names are quoted (or not) when dragging and dropping.
There are five possible values:

* `"None"` - no quoting is performed, the file name is passed through as-is.
* `"SpacesOnly"` - backslash-escape only spaces, leaving all other characters as-is.  This is the default for non-Windows platforms.
* `"Posix"` - use POSIX style shell word escaping.
* `"Windows"` - use Windows style shell word escaping: double-quote filename with space characters in it, and leaving others as-is. This is the default on Windows.
* `"WindowsAlwaysQuoted"` - like `"Windows"`, while always double-quote the filename.

For example:

| `quote_dropped_files`   | file name        | quoted result       |
|-------------------------|------------------|---------------------|
| `"None"`                | `hello ($world)` | `hello ($world)`    |
| `"SpacesOnly"`          | `hello ($world)` | `hello\ ($world)`   |
| `"Posix"`               | `hello ($world)` | `"hello (\$world)"` |
| `"Windows"`             | `hello ($world)` | `"hello ($world)"`  |
| `"WindowsAlwaysQuoted"` | `hello ($world)` | `"hello ($world)"`  |

Drag and drop support for files is a platform dependent feature

|Platform  |Supported since    |
|----------|-------------------|
|macOS     |nightly builds only|
|Windows   |nightly builds only|
|X11       |Not yet            |
|Wayland   |Not yet            |
