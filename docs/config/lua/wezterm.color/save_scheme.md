# `wezterm.color.save_scheme(colors, metadata, file_name)`

{{since('20220807-113146-c2fee766')}}

Saves a color scheme as a wezterm TOML file.
This is useful when sharing your custom color scheme with others.
While you could share the lua representation of the scheme, the
TOML file is recommended for sharing as it is purely declarative:
no executable logic is present in the TOML color scheme which makes
it safe to consume "random" schemes from the internet.

This example demonstrates importing a base16 scheme and exporting
it as a wezterm scheme.

Given a yaml file with these contents:

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
> wezterm.color.save_scheme(colors, metadata, "/tmp/cupcacke.toml")
```

produces a toml file with these contents:

```toml
[colors]
ansi = [
    '#fbf1f2',
    '#d57e85',
    '#a3b367',
    '#dcb16c',
    '#7297b9',
    '#bb99b4',
    '#69a9a7',
    '#8b8198',
]
background = '#fbf1f2'
brights = [
    '#bfb9c6',
    '#d57e85',
    '#a3b367',
    '#dcb16c',
    '#7297b9',
    '#bb99b4',
    '#69a9a7',
    '#585062',
]
cursor_bg = '#8b8198'
cursor_border = '#8b8198'
cursor_fg = '#8b8198'
foreground = '#8b8198'
selection_bg = '#8b8198'
selection_fg = '#fbf1f2'

[colors.indexed]

[metadata]
author = 'Chris Kempson (http://chriskempson.com)'
name = 'Cupcake'
```
