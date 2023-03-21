# `wezterm.color.load_terminal_sexy_scheme(file_name)`

{{since('20220807-113146-c2fee766')}}

Loads a json file exported from [terminal.sexy](https://terminal.sexy/)
and returns it as a wezterm color scheme.

Note that wezterm ships with all of the pre-defined terminal.sexy color
schemes, so this function is primarily useful if you want to design a color
scheme using terminal.sexy and then import it to wezterm.

This function returns a tuple of the the color definitions and the metadata.

For example, given a json file with these contents:

```json
{
  "name": "",
  "author": "",
  "color": [
    "#282a2e",
    "#a54242",
    "#8c9440",
    "#de935f",
    "#5f819d",
    "#85678f",
    "#5e8d87",
    "#707880",
    "#373b41",
    "#cc6666",
    "#b5bd68",
    "#f0c674",
    "#81a2be",
    "#b294bb",
    "#8abeb7",
    "#c5c8c6"
  ],
  "foreground": "#c5c8c6",
  "background": "#1d1f21"
}
```

Then:

```
> colors, metadata = wezterm.color.load_terminal_sexy_scheme("/path/to/file.json")
> print(colors)
22:37:10.416 INFO logging > lua: {
    "ansi": [
      "#282a2e",
      "#a54242",
      "#8c9440",
      "#de935f",
      "#5f819d",
      "#85678f",
      "#5e8d87",
      "#707880",
    ],
    "background": "#1d1f21",
    "brights": [
      "#373b41",
      "#cc6666",
      "#b5bd68",
      "#f0c674",
      "#81a2be",
      "#b294bb",
      "#8abeb7",
      "#c5c8c6"
    ],
    "foreground": "#c5c8c6",
}
> print(metadata)
22:37:06.041 INFO logging > lua: {
    "name": "",
    "author": ""
}
```
