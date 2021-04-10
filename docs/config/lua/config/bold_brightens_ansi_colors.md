# `bold_brightens_ansi_colors = true`

When true (the default), PaletteIndex 0-7 are shifted to bright when the font
intensity is bold.

This brightening effect doesn't occur when the text is set
to the default foreground color!

This defaults to true for better compatibility with a wide
range of mature software; for instance, a lot of software
assumes that Black+Bold renders as a Dark Grey which is
legible on a Black background, but if this option is set to
false, it would render as Black on Black.
