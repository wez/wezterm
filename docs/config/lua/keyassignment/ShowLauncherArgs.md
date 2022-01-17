# ShowLauncherArgs

*Since: nightly builds only*

Activate the [Launcher Menu](../../launch.md#the-launcher-menu)
in the current tab, scoping it to a set of items and with an optional title.

The arguments are a lua table with the following keys:

* `flags` - required; the set of flags that specifies what to show in the launcher
* `title` - optional; the title to show in the tab while the launcher is active

The possible flags are:

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

The flags can be joined together using a `|` character, so `"TABS|DOMAINS"` is
an example of a set of flags that will include both tabs and domains in the
list.

This example shows how to make `ALT-9` activate the launcher directly in fuzzy
matching mode, and have it show only tabs:

```lua
local wezterm = require 'wezterm'

return {
  keys = {
    {key="9", mods="ALT", action=wezterm.action{ShowLauncherArgs={flags="FUZZY|TABS"}}},
  },
}
```

