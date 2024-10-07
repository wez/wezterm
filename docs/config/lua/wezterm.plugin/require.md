# `wezterm.plugin.require()`

{{since('???')}}

Takes a `url` string as argument, which it will use to clone the git repository if not already cloned, and then require the lua files.

It will do so by making a directory based on the `url` but with `/` and `\` replaced by `sZs`, `:` replaced by `sCs` and `.` replaced by `sDs`.
It will place this directory in the plugins directory and then clone the repository into it.

It will try to require `init.lua` from the repository root or in the `plugin/` sub directory.

It is recommended to use [require_as_alias](./require_as_alias.md) instead to make it easy to require lua files via the alias instead of the modified url name.

