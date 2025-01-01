# `wezterm.color.load_base16_scheme(file_name)`

{{since('20220807-113146-c2fee766')}}

Loads a yaml file in [base16](https://github.com/chriskempson/base16) format
and returns it as a wezterm color scheme.

Note that wezterm ships with the base16 color schemes that were referenced via
[base16-schemes-source](https://github.com/chriskempson/base16-schemes-source)
when the release was prepared so this function is primarily useful if you want
to import a base16 color scheme that either isn't listed from the main list, or
that was created after your version of wezterm was built.

This function returns a tuple of the the color definitions and the metadata.

For example, given a yaml file with these contents:

```yaml
scheme: "Cupcake"
author: "Chris Kempson (http://chriskempson.com)"
base00: "fbf1f2"
base01: "f2f1f4"
base02: "d8d5dd"
base03: "bfb9c6"
base04: "a59daf"
base05: "8b8198"
base06: "72677E"
base07: "585062"
base08: "D57E85"
base09: "EBB790"
base0A: "DCB16C"
base0B: "A3B367"
base0C: "69A9A7"
base0D: "7297B9"
base0E: "BB99B4"
base0F: "BAA58C"
```

Then:

```
> colors, metadata = wezterm.color.load_base16_scheme("/tmp/cupcake.yaml")
> print(colors)
22:59:26.998 INFO logging > lua: {
    "ansi": [
        "#fbf1f2",
        "#d57e85",
        "#a3b367",
        "#dcb16c",
        "#7297b9",
        "#bb99b4",
        "#69a9a7",
        "#8b8198",
    ],
    "background": "#fbf1f2",
    "brights": [
        "#bfb9c6",
        "#d57e85",
        "#a3b367",
        "#dcb16c",
        "#7297b9",
        "#bb99b4",
        "#69a9a7",
        "#585062",
    ],
    "cursor_bg": "#8b8198",
    "cursor_border": "#8b8198",
    "cursor_fg": "#8b8198",
    "foreground": "#8b8198",
    "indexed": {},
    "selection_bg": "#8b8198",
    "selection_fg": "#fbf1f2",
}
> print(metadata)
22:59:29.671 INFO logging > lua: {
    "author": "Chris Kempson (http://chriskempson.com)",
    "name": "Cupcake",
}
```
