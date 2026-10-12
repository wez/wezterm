---
tags:
  - search
---

# `Search`

{{since('20200607-144723-74889cd4')}}

This action will trigger the search overlay for the current tab.  It accepts a
typed pattern string as its parameter, allowing for `Regex`,
`CaseSensitiveString`, `CaseInSensitiveString` and `CaseSmartString` as pattern
matching types.

For `Regex` matching, the supported [regular expression syntax is described here](https://docs.rs/regex/1.3.9/regex/#syntax).

{{since('nightly')}}

`CaseSmartString` was added, to make the search case-_insensitive_ until the search term
includes any uppercase character, which triggers the search to become case-_sensitive_.

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
  -- search for the string "hash" matching regardless of case, until the search string is edited
  -- with an uppercase and the matching becomes case sensitive.
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search { CaseSmartString = 'hash' },
  },
}
```

[Learn more about the search overlay](../../../scrollback.md#searching-the-scrollback)

{{since('20220624-141144-bd1b7c5d')}}

You may now use `wezterm.action.Search("CurrentSelectionOrEmptyString")` to have the search take the currently selected text as the item to search.

The selection text is adjusted to be a single line.

{{since('nightly')}}

`Search` now accepts a table with additional configuration options. The simple
syntax shown above continues to work for basic searches.

| Field | Required | Description |
|-------|----------|-------------|
| `pattern` | Yes | The search pattern. Accepts the same values as the simple syntax: `{ Regex = '...' }`, `{ CaseSensitiveString = '...' }`, `{ CaseInSensitiveString = '...' }`, or `"CurrentSelectionOrEmptyString"` |
| `activate_match` | No | Which match to activate when the search overlay opens. Defaults to `"First"` |

The `activate_match` field accepts:
* `"First"` - activate the first match in the scrollback
* `"AfterCursor"` - activate the first match after the current cursor position
* `"BeforeCursor"` - activate the first match before the current cursor position

```lua
local act = wezterm.action

config.keys = {
  -- search for git hashes, activating the first match after cursor
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search {
      pattern = { Regex = '[a-f0-9]{6,}' },
      activate_match = 'AfterCursor',
    },
  },
  -- search using current selection, activating the first match before cursor
  {
    key = 'H',
    mods = 'SHIFT|CTRL',
    action = act.Search {
      pattern = 'CurrentSelectionOrEmptyString',
      activate_match = 'BeforeCursor',
    },
  },
}
```
