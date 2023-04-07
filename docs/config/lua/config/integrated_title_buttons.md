# `integrated_title_button_style = BUTTONS`

{{since('nightly')}}

Configures the ordering and set of window management buttons to show when
`window_decorations = "INTEGRATED_BUTTONS|RESIZE"`.

The value is a table listing the buttons. Each element can have one of
the following values:

* `"Hide"` - the window hide or minimize button
* `"Maximize"` - the window maximize button
* `"Close"` - the window close button

The default value is equivalent to:

```lua
config.integrated_title_buttons = { 'Hide', 'Maximize', 'Close' }
```

You can change the order by listing them in a different order:

```lua
config.integrated_title_buttons = { 'Close', 'Maximize', 'Hide' }
```

or remove buttons you don't want:

```lua
config.integrated_title_buttons = { 'Close' }
```

