## window:tabs_with_info()

{{since('20220807-113146-c2fee766')}}

Returns an array table holding an extended info entry for each of the tabs
contained within this window.

Each element is a lua table with the following fields:

* `index` - the 0-based tab index
* `is_active` - a boolean indicating whether this is the active tab within the window
* `tab` - the [MuxTab](../MuxTab/index.md) object

