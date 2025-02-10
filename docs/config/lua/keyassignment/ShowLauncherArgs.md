# `ShowLauncherArgs`

{{since('20220319-142410-0fcdea07')}}

Activate the [Launcher Menu](../../launch.md#the-launcher-menu)
in the current tab, scoping it to a set of items and with an optional title.

The arguments are a lua table with the following keys:

* `flags` - required; the set of flags that specifies what to show in the launcher
* `title` - optional; the title to show in the tab while the launcher is active
* `help_text` - a string to display when in the default mode. Defaults to:
  `"Select an item and press Enter=launch  Esc=cancel  /=filter"` {{since('nightly', inline=True)}}
* `fuzzy_help_text` - a string to display when in fuzzy finding mode. Defaults to:
  `"Fuzzy matching: "` {{since('nightly', inline=True)}}

The possible flags are listed below. You must explicitly list each item that you
want to include in the launcher. If you only specify `"FUZZY"` then you will see
an empty launcher:

* `"FUZZY"` - activate in fuzzy-only mode. By default the launcher will allow
  using the number keys to select from the first few items, as well as *vi* movement
  keys to select items. Pressing `/` will enter fuzzy filtering mode, allowing you
  to type a search term and reduce the set of matches.
  When you use the `"FUZZY"` flag, the launcher activates directly in fuzzy filtering
  mode.
* `"TABS"` - include the list of tabs from the current window
* `"LAUNCH_MENU_ITEMS"` - include the [launch_menu](../config/launch_menu.md) items
* `"DOMAINS"` - include multiplexing domains
* `"KEY_ASSIGNMENTS"` - include items taken from your key assignments
* `"WORKSPACES"` - include workspaces
* `"COMMANDS"` - include a number of default commands {{since('20220408-101518-b908e2dd', inline=True)}}

The flags can be joined together using a `|` character, so `"TABS|DOMAINS"` is
an example of a set of flags that will include both tabs and domains in the
list.

This example shows how to make `ALT-9` activate the launcher directly in fuzzy
matching mode, and have it show only tabs:

```lua
config.keys = {
  {
    key = '9',
    mods = 'ALT',
    action = wezterm.action.ShowLauncherArgs { flags = 'FUZZY|TABS' },
  },
}
```

