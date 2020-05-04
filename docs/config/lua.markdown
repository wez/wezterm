## Lua Reference

This section documents the various lua functions that are provided to
the configuration file.  These are provided by the `wezterm` module that
must be imported into your configuration file:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font("JetBrains Mono"),
}
```

### Making your own Lua Modules

If you'd like to break apart your configuration into multiple files, you'll
be interested in this information.

The `package.path` is configured with the following paths in this order:

* On Windows: a `wezterm_modules` dir in the same directory as `wezterm.exe`
* `~/.config/wezterm`
* `~/.wezterm`
* A system specific set of paths which may (or may not!) find locally installed lua modules

That means that if you wanted to break your config up into a `helpers.lua` file
you would place it in `~/.config/wezterm/helpers.lua` with contents like this:

```lua
-- I am helpers.lua and I should live in ~/.config/wezterm/helpers.lua

local wezterm = require 'wezterm';

-- This is the module table that we will export
local module = {}

-- This function is private to this module and is not visible
-- outside.
local function private_helper()
  wezterm.log_error("hello!")
end

-- define a function in the module table.
-- Only functions defined in `module` will be exported to
-- code that imports this module
function module.my_function()
  private_helper()
end

-- return our module table
return module
```

and then in your `wezterm.lua`
you would use it like this:

```lua
local helpers = require 'helpers';
helpers.my_function()
```

### `wezterm.config_dir`

This constant is set to the path to the directory in which your `wezterm.lua`
configuration file was found.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Config Dir " .. wezterm.config_dir)
```

### `wezterm.target_triple`

This constant is set to the [Rust target
triple](https://forge.rust-lang.org/release/platform-support.html) for the
platform on which `wezterm` was built.  This can be useful when you wish to
conditionally adjust your configuration based on the platform.

```lua
local wezterm = require 'wezterm';

if wezterm.target_triple == "x86_64-pc-windows-msvc" then
  -- We are running on Windows; maybe we emit different
  -- key assignments here?
end
```

The most common triples are:

* `x86_64-pc-windows-msvc` - Windows
* `x86_64-apple-darwin` - macOS
* `x86_64-unknown-linux-gnu` - Linux

### `wezterm.version`

This constant is set to the `wezterm` version string that is also reported
by running `wezterm -V`.  This can potentially be used to adjust configuration
according to the installed version.

The version string looks like `20200406-151651-5b700e4`.  You can compare the
strings lexicographically if you wish to test whether a given version is newer
than another; the first component is the date on which the release was made,
the second component is the time and the final component is a git hash.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Version " .. wezterm.version)
```

### `wezterm.home_dir`

This constant is set to the home directory of the user running `wezterm`.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Home " .. wezterm.home_dir)
```

### `wezterm.running_under_wsl()`

This function returns a boolean indicating whether we believe that we are
running in a Windows Services for Linux (WSL) container.  In such an
environment the `wezterm.target_triple` will indicate that we are running in
Linux but there will be some slight differences in system behavior (such as
filesystem capabilities) that you may wish to probe for in the configuration.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("System " .. wezterm.target_triple .. " " ..
  tostring(wezterm.running_under_wsl()))
```

### `wezterm.log_error(msg)`

This function logs the provided message string through wezterm's logging layer.
If you started wezterm from a terminal that text will print to the stdout of
that terminal.  If running as a daemon for the multiplexer server then it will
be logged to the daemon output path.

```lua
local wezterm = require 'wezterm';
wezterm.log_error("Hello!");
```

### `wezterm.font(family [, attributes])`

This function constructs a lua table that corresponds to the internal `FontAttributes`
struct that is used to select a single named font:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono"),
}
```

The second parameter is an optional table that can be used to specify some
attributes; the following keys are allowed:

* `bold` - whether to select a bold variant of the font (default: `false`)
* `italic` - whether to select an italic variant of the font (default: `false`)

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("JetBrains Mono", {bold=true}),
}
```

### `wezterm.font_with_fallback(families [, attributes])`

This function constructs a lua table that configures a font with fallback processing.
Glyphs are looked up in the first font in the list but if missing the next font is
checked and so on.

The first parameter is a table listing the fonts in their preferred order:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font_with_fallback({"JetBrains Mono", "Noto Color Emoji"}),
}
```

The second parameter behaves the same as that of `wezterm.font`.

### `wezterm.hostname()`

This function returns the current hostname of the system that is running wezterm.
This can be useful to adjust configuration based on the host.

Note that environments that use DHCP and have many clients and short leases may
make it harder to rely on the hostname for this purpose.

```lua
local wezterm = require 'wezterm';
local hostname = wezterm.hostname();

local font_size;
if hostname == "pixelbookgo-localdomain" then
  -- Use a bigger font on the smaller screen of my PixelBook Go
  font_size = 12.0;
else
  font_size = 10.0;
end

return {
  font_size = font_size
}
```

### `wezterm.read_dir(path)`

*Since: 20200503-171512-b13ef15f*

This function returns an array containing the absolute file names of the
directory specified.  Due to limitations in the lua bindings, all of the paths
must be able to be represented as UTF-8 or this function will generate an
error.

```lua
local wezterm = require 'wezterm';

-- logs the names of all of the entries under `/etc`
for _, v in ipairs(wezterm.read_dir("/etc")) do
  wezterm.log_error("entry: " .. v)
end
```

### `wezterm.glob(pattern [, relative_to])`

*Since: 20200503-171512-b13ef15f*

This function evalutes the glob `pattern` and returns an array containing the
absolute file names of the matching results.  Due to limitations in the lua
bindings, all of the paths must be able to be represented as UTF-8 or this
function will generate an error.

The optional `relative_to` parameter can be used to make the results relative
to a path.  If the results have the same prefix as `relative_to` then it will
be removed from the returned path.

```lua
local wezterm = require 'wezterm';

-- logs the names of all of the conf files under `/etc`
for _, v in ipairs(wezterm.glob("/etc/*.conf")) do
  wezterm.log_error("entry: " .. v)
end
```

### `wezterm.run_child_process(args)`

*Since: 20200503-171512-b13ef15f*

This function accepts an argument list; it will attempt to spawn that command
and will return a tuple consisting of the boolean success of the invocation,
the stdout data and the stderr data.

```lua
local wezterm = require 'wezterm';

local success, stdout, stderr = wezterm.run_child_process({"ls", "-l"})
```

### `wezterm.split_by_newlines(str)`

*Since: 20200503-171512-b13ef15f*

This function takes the input string and splits it by newlines (both `\n` and `\r\n`
are recognized as newlines) and returns the result as an array of strings that
have the newlines removed.

```lua
local wezterm = require 'wezterm';

local example = "hello\nthere\n";

for _, line in ipairs(wezterm.split_by_newlines(example)) do
  wezterm.log_error(line)
end
```

### `wezterm.utf16_to_utf8(str)`

*Since: 20200503-171512-b13ef15f*

This function is overly specific and exists primarily to workaround
[this wsl.exe issue](https://github.com/microsoft/WSL/issues/4456).

It takes as input a string and attempts to convert it from utf16 to utf8.

```lua
local wezterm = require 'wezterm';

local success, wsl_list, wsl_err = wezterm.run_child_process({"wsl.exe", "-l"})
wsl_list = wezterm.utf16_to_utf8(wsl_list)
```

