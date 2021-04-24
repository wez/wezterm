# TabInformation

The `TabInformation` struct describes a tab.  `TabInformation` is purely a
snapshot of some of the key characteristics of the tab, intended for use in
synchronous, fast, event callbacks that format GUI elements such as the window
and tab title bars.

The `TabInformation` struct contains the following fields:

* `tab_id` - the identifier for the tab
* `tab_index` - the logical tab position within its containing window, with 0 indicating the leftmost tab
* `is_active` - is true if this tab is the active tab

