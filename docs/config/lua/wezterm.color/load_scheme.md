# `wezterm.color.load_scheme(file_name)`

{{since('20220807-113146-c2fee766')}}

Loads a wezterm color scheme from a TOML file.  This function
returns a tuple of the the color definitions and the metadata:

```
> colors, metadata = wezterm.color.load_scheme("wezterm/assets/colors/Abernathy.toml")
> print(metadata)
22:37:06.041 INFO logging > lua: {
    "name": "Abernathy",
    "origin_url": "https://github.com/mbadolato/iTerm2-Color-Schemes",
}
> print(colors)
22:37:10.416 INFO logging > lua: {
    "ansi": [
        "#000000",
        "#cd0000",
        "#00cd00",
        "#cdcd00",
        "#1093f5",
        "#cd00cd",
        "#00cdcd",
        "#faebd7",
    ],
    "background": "#111416",
    "brights": [
        "#404040",
        "#ff0000",
        "#00ff00",
        "#ffff00",
        "#11b5f6",
        "#ff00ff",
        "#00ffff",
        "#ffffff",
    ],
    "cursor_bg": "#bbbbbb",
    "cursor_border": "#bbbbbb",
    "cursor_fg": "#ffffff",
    "foreground": "#eeeeec",
    "indexed": {},
    "selection_bg": "#eeeeec",
    "selection_fg": "#333333",
}
```
