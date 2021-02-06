### Misc configuration

```lua
return {
  -- How many lines of scrollback you want to retain per tab
  scrollback_lines = 3500,

  -- Enable the scrollbar.  This is currently disabled by default.
  -- It will occupy the right window padding space.
  -- If right padding is set to 0 then it will be increased
  -- to a single cell width
  enable_scroll_bar = true,

  -- What to set the TERM variable to
  term = "xterm-256color",

  -- Constrains the rate at which the multiplexer server will
  -- unilaterally push data to the client.
  -- This helps to avoid saturating the link between the client
  -- and server.
  -- Each time the screen is updated as a result of the child
  -- command outputting data (rather than in response to input
  -- from the client), the server considers whether to push
  -- the result to the client.
  -- That decision is throttled by this configuration value
  -- which has a default value of 10/s
  ratelimit_mux_output_pushes_per_second = 10,

  -- Constrain how often the mux server scans the terminal
  -- model to compute a diff to send to the mux client.
  -- The default value is 100/s
  ratelimit_mux_output_scans_per_second = 100,

  -- If false, do not try to use a Wayland protocol connection
  -- when starting the gui frontend, and instead use X11.
  -- This option is only considered on X11/Wayland systems and
  -- has no effect on macOS or Windows.
  -- The default is true.
  enable_wayland = true,

  -- Specifies how often a blinking cursor transitions between visible
  -- and invisible, expressed in milliseconds.
  -- Setting this to 0 disables blinking.
  -- Note that this value is approximate due to the way that the system
  -- event loop schedulers manage timers; non-zero values will be at
  -- least the interval specified with some degree of slop.
  -- It is recommended to avoid blinking cursors when on battery power,
  -- as it is relatively costly to keep re-rendering for the blink!
  cursor_blink_rate = 800,

  -- Specifies the default cursor style.  various escape sequences
  -- can override the default style in different situations (eg:
  -- an editor can change it depending on the mode), but this value
  -- controls how the cursor appears when it is reset to default.
  -- The default is `SteadyBlock`.
  -- Acceptable values are `SteadyBlock`, `BlinkingBlock`,
  -- `SteadyUnderline`, `BlinkingUnderline`, `SteadyBar`,
  -- and `BlinkingBar`.
  default_cursor_style = "SteadyBlock",

  -- Specifies the maximum width that a tab can have in the
  -- tab bar.  Defaults to 16 glyphs in width.
  tab_max_width = 16,

  -- Set this to "NeverPrompt" if you don't like confirming closing
  -- windows every time
  window_close_confirmation = "AlwaysPrompt",

  -- When true (the default), watch the config file and reload it
  -- automatically when it is detected as changing.
  -- When false, you will need to manually trigger a config reload.
  automatically_reload_config = true,

  -- The github releases section is checked regularly for updated stable
  -- releases and show a simple UI to let you know about the update,
  -- with links to download/install it. It doesn't automatically download
  -- the release. No data are collected for the wezterm project as part of this.
  -- By default it is checked once every 24 hours.
  -- Set `check_for_updates` to `false` to disable this completely or set
  -- `check_for_updates_interval_seconds` for an alternative update interval.
  check_for_updates = true,
  check_for_updates_interval_seconds = 86400,
}
```

