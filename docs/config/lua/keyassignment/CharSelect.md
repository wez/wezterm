# `CharSelect`

{{since('20220903-194523-3bb1ed61')}}

Activates *Character Selection Mode*, which is a pop-over modal that allows you
to browse characters by category as well as fuzzy search by name or hex unicode
codepoint value.

Characters are categorized into the following groups:

* `"RecentlyUsed"` - recently selected characters, ordered by [frecency](https://en.wikipedia.org/wiki/Frecency)
* `"SmileysAndEmotion"`
* `"PeopleAndBody"`
* `"AnimalsAndNature"`
* `"FoodAndDrink"`
* `"TravelAndPlaces"`
* `"Activities"`
* `"Objects"`
* `"Symbols"`
* `"Flags"`
* `"NerdFonts"` - glyphs that are present in [Nerd Fonts](https://www.nerdfonts.com/cheat-sheet)
* `"UnicodeNames"` - all codepoints defined in unicode

The following key assignments are available (they are not currently configurable):

|Key             | Action |
|----------------|--------|
|UpArrow         |Move Up |
|DownArrow       |Move Down|
|Enter           |Accept the current item, copy it to the clipboard, insert it into the active pane, and cancel the modal|
|Esc             |Cancel the modal|
|CTRL-g          |Cancel the modal|
|CTRL-r          |Cycle to the next group of characters|
|CTRL-SHIFT-r    |Cycle to the previous group of characters|
|CTRL-u          |Clear text input|

Typing a name or a hex unicode codepoint value will fuzzy search across all
possible groups (not just the current group) and filter the results.

This action is by default assigned to `CTRL-SHIFT-U` (`U` for `Unicode`).

The default assignment is equivalent to this config:

```lua
-- Control the size of the font.
-- Uses the same font as window_frame.font
-- char_select_font_size = 18.0,

config.keys = {
  {
    key = 'u',
    mods = 'SHIFT|CTRL',
    action = wezterm.action.CharSelect {
      copy_on_select = true,
      copy_to = 'ClipboardAndPrimarySelection',
    },
  },
}
```

The `CharSelect` action accepts a lua table with the following fields:

* `copy_on_select` - a boolean that controls whether hitting `Enter` to select
  an item will copy to the clipboard, in addition to sending the item to the
  active pane. The default is `true`, but you can set it to `false` if you
  prefer.
* `copy_to` - allows you to control where the item will be copied to. Accepts
  the same values as [CopyTo](CopyTo.md). The default is
  `'ClipboardAndPrimarySelection'`.
* `group` - an optional group to pre-select. You may use any of the groups
  listed above (eg: `"SmileysAndEmotion"`). If omitted, wezterm will default to
  `"RecentlyUsed"` if you have previously selected an item, or
  `"SmileysAndEmotion"` otherwise.

See also:
* [char_select_font_size](../config/char_select_font_size.md)
* [char_select_fg_color](../config/char_select_fg_color.md)
* [char_select_bg_color](../config/char_select_bg_color.md)
