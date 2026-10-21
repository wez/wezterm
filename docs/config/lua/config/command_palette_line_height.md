---
tags:
  - font
  - command_palette
---
# `command_palette_line_height = 1.0`

{{since('nightly')}}

Scales the computed line height used by
[ActivateCommandPalette](../keyassignment/ActivateCommandPalette.md).
The default is `1.0`, which uses the font-specified metrics.

If the command palette feels too vertically cramped then you can set
`command_palette_line_height = 1.2` to increase the vertical spacing by 20%.
Conversely, setting `command_palette_line_height = 0.9` will decrease the
vertical spacing by 10%.

```lua
config.command_palette_line_height = 1.2
```

See also [line_height](line_height.md), which only applies to terminal cells.
