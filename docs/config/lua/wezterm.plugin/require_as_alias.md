# `wezterm.plugin.require_as_alias()`

{{since('???')}}

Takes a `url` string and a `alias` string as argument. The alias cannot contain `\`, `/`, `:` or `,`.

It will make a sub directory with the name of the alias in the plugins sub directory and then clone the git repository from the url into this directory.

It is assumed that the lua files are placed in a `init.lua` in the repository root or in the `plugin/` sub-directory of the repository root.

