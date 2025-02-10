# `QuickSelectArgs`

{{since('20220101-133340-7edc5b5a')}}

Activates [Quick Select Mode](../../../quickselect.md) but with the option
to override the global configuration.

This example shows how to pop up a quick select that is scoped solely to
a very basic http regex; it will only match those regexes regardless of
the default or the [quick_select_patterns](../config/quick_select_patterns.md)
configuration:

```lua
local wezterm = require 'wezterm'

config.keys = {
  {
    key = 'P',
    mods = 'CTRL',
    action = wezterm.action.QuickSelectArgs {
      patterns = {
        'https?://\\S+',
      },
    },
  },
}
```

The `QuickSelectArgs` struct allows for the following fields:

* `patterns` - if present, completely overrides the normal set of patterns and uses only the patterns specified
* `alphabet` - if present, this alphabet is used instead of [quick_select_alphabet](../config/quick_select_alphabet.md)
* `action` - if present, this key assignment action is performed as if by [window:perform_action](../window/perform_action.md) when an item is selected.  The normal clipboard action is NOT performed in this case.
* `skip_action_on_paste` - overrides whether `action` is performed after an item is selected using a capital value (when paste occurs). {{since('nightly', inline=True)}}
* `label` - if present, replaces the string `"copy"` that is shown at the bottom of the overlay; you can use this to indicate which action will happen if you are using `action`.
* `scope_lines` - Specify the number of lines to search above and below the current viewport. The default is 1000 lines. The scope will be increased to the current viewport height if it is smaller than the viewport. {{since('20220807-113146-c2fee766', inline=True)}}. In earlier releases, the entire scrollback was always searched).

Here's an example that shows how to trigger some lua code to operate on the
quick-selected text, instead of copying it to the clipboard.  Here, we open
the selected URL using the web browser:

```lua
local wezterm = require 'wezterm'

config.keys = {
  {
    key = 'P',
    mods = 'CTRL',
    action = wezterm.action.QuickSelectArgs {
      label = 'open url',
      patterns = {
        'https?://\\S+',
      },
      skip_action_on_paste = true,
      action = wezterm.action_callback(function(window, pane)
        local url = window:get_selection_text_for_pane(pane)
        wezterm.log_info('opening: ' .. url)
        wezterm.open_with(url)
      end),
    },
  },
}
```

See also [wezterm.open_with](../wezterm/open_with.md).
