# Search

*since: 20200607-144723-74889cd4*

This action will trigger the search overlay for the current tab.
It accepts a typed pattern string as its parameter, allowing for
`Regex`, `CaseSensitiveString` and `CaseInSensitiveString` as
pattern matching types.

The supported [regular expression syntax is described
here](https://docs.rs/regex/1.3.9/regex/#syntax).


```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- search for things that look like git hashes
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={Regex="[a-f0-9]{6,}"}}},
    -- search for the lowercase string "hash" matching the case exactly
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={CaseSensitiveString="hash"}}},
    -- search for the string "hash" matching regardless of case
    {key="H", mods="SHIFT|CTRL", action=wezterm.action{Search={CaseInSensitiveString="hash"}}},
  },
}
```


