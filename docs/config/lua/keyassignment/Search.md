# `Search`

{{since('20200607-144723-74889cd4')}}

This action will trigger the search overlay for the current tab.
It accepts a typed pattern string as its parameter, allowing for
`Regex`, `CaseSensitiveString` and `CaseInSensitiveString` as
pattern matching types.

The supported [regular expression syntax is described
here](https://docs.rs/regex/1.3.9/regex/#syntax).


```lua
local act = wezterm.action

config.keys = {
  -- search for things that look like git hashes
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search {
      Regex = '[a-f0-9]{6,}',
    },
  },
  -- search for the lowercase string "hash" matching the case exactly
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search { CaseSensitiveString = 'hash' },
  },
  -- search for the string "hash" matching regardless of case
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search { CaseInSensitiveString = 'hash' },
  },
}
```

[Learn more about the search overlay](../../../scrollback.md#searching-the-scrollback)

{{since('20220624-141144-bd1b7c5d')}}

You may now use `wezterm.action.Search("CurrentSelectionOrEmptyString")` to have the search take the currently selected text as the item to search.

The selection text is adjusted to be a single line.
