# `wezterm.gui.screens()`

{{since('20220807-113146-c2fee766')}}

Returns information about the screens connected to the system.

The follow example was typed into the [Debug
Overlay](../keyassignment/ShowDebugOverlay.md) (by default: press
`CTRL-SHIFT-L`) on a macbook:

```
> wezterm.gui.screens()
{
    "active": {
        "height": 1800,
        "name": "Built-in Retina Display",
        "width": 2880,
        "x": 0,
        "y": 0,
    },
    "by_name": {
        "Built-in Retina Display": {
            "height": 1800,
            "name": "Built-in Retina Display",
            "width": 2880,
            "x": 0,
            "y": 0,
        },
    },
    "main": {
        "height": 1800,
        "name": "Built-in Retina Display",
        "width": 2880,
        "x": 0,
        "y": 0,
    },
    "origin_x": 0,
    "origin_y": 0,
    "virtual_height": 1800,
    "virtual_width": 2880,
}
```

The return value is a table with the following keys:

* `active` - contains information about the *active* screen. The active screen is the one which has input focus. On some systems, wezterm will return the same information as the `main` screen screen.
* `main` - contains information about the *main* screen. The main screen is the primary screen: the one that has the menu bar or task bar.
* `by_name` - a table containing information about each screen, indexed by their name
* `origin_x`, `origin_y`, `virtual_height`, `virtual_width` - the bounds of the combined desktop geometry spanning all connected screens

The screen information is a table with the following keys:

* `name` - the name of the screen.
* `x`, `y`, `width`, `height` - the bounds of this screen
* `max_fps` - the maximum refresh rate supported by the screen, if known, or `nil` otherwise. {{since('20220903-194523-3bb1ed61', inline=True)}}
