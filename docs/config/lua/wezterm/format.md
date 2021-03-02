# `wezterm.format({})`

*Since: nightly*

`wezterm.format` can be used to produce a formatted string
with terminal graphic attributes such as bold, italic and colors.
The resultant string is rendered into a string with wezterm
compatible escape sequences embedded.

`wezterm.format` accepts a single array argument, where each
element is a `FormatItem`.

This example logs the text `Hello`, then the date/time, underlined, in purple
text on a blue background to the stderr of the wezterm process:

```lua
local wezterm = require 'wezterm';

local success, date, stderr = wezterm.run_child_process({"date"});

wezterm.log_info(wezterm.format({
  {Attribute={Underline="Single"}},
  {Foreground={AnsiColor="Fuschia"}},
  {Background={Color="blue"}},
  {Text="Hello " .. date},
}))
```

Possible values for the `FormatItem` elements are:

* `{Text="Hello"}` - the text `Hello`. The string can be any string expression.
* `{Attribute={Underline="None"}}` - disable underline
* `{Attribute={Underline="Single"}}` - enable single underline
* `{Attribute={Underline="Double"}}` - enable double underline
* `{Attribute={Underline="Curly"}}` - enable curly underline
* `{Attribute={Underline="Dotted"}}` - enable dotted underline
* `{Attribute={Underline="Dashed"}}` - enable dashed underline
* `{Attribute={Intensity="Normal"}}` - set normal intensity
* `{Attribute={Intensity="Bold"}}` - set bold intensity
* `{Attribute={Intensity="Half"}}` - set half intensity
* `{Attribute={Italic=true}}` - enable italics
* `{Attribute={Italic=false}}` - disable italics
* `{Foreground={AnsiColor="Black"}}` - set foreground color to one of the ansi color palette values (index 0-15) using one of the names `Black`, `Maroon`, `Green`, `Olive`, `Navy`, `Purple`, `Teal`, `Silver`, `Grey`, `Red`, `Lime`, `Yellow`, `Blue`, `Fuschia`, `Aqua` or `White`.
* `{Foreground={Color="yellow"}}` - set foreground color to a named color or rgb value like `#ffffff`.
* `{Background={AnsiColor="Black"}}` - set the background color to an ansi color as per `Foreground` above.
* `{Background={Color="blue"}}` - set the background color to a named color or rgb value as per `Foreground` above.

