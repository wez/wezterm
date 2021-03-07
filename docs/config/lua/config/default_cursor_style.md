# `default_cursor_style = "SteadyBlock"`

Specifies the default cursor style.  Various escape sequences
can override the default style in different situations (eg:
an editor can change it depending on the mode), but this value
controls how the cursor appears when it is reset to default.
The default is `SteadyBlock`.

Acceptable values are `SteadyBlock`, `BlinkingBlock`,
`SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
and `BlinkingBar`.

```lua
return {
  default_cursor_style = "SteadyBlock",
}
```

