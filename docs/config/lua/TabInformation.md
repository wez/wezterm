# TabInformation

The `TabInformation` struct describes a tab.  `TabInformation` is purely a
snapshot of some of the key characteristics of the tab, intended for use in
synchronous, fast, event callbacks that format GUI elements such as the window
and tab title bars.

The `TabInformation` struct contains the following fields:

* `tab_id` - the identifier for the tab
* `tab_index` - the logical tab position within its containing window, with 0 indicating the leftmost tab
* `is_active` - is true if this tab is the active tab
* `active_pane` - the [PaneInformation](PaneInformation.md) for the active pane in this tab
* `window_id` - the ID of the window that contains this tab {{since('20220807-113146-c2fee766', inline=True)}}
* `window_title` - the title of the window that contains this tab {{since('20220807-113146-c2fee766', inline=True)}}
* `tab_title` - the title of the tab {{since('20220807-113146-c2fee766', inline=True)}}


