# `window-config-reloaded`

{{since('20210314-114017-04b7cedd')}}

The `window-config-reloaded` event is emitted when the configuration for a
window has been reloaded.  This can occur when the configuration file is
detected as changed (when
[automatically_reload_config](../config/automatically_reload_config.md) is
enabled), when the configuration is explicitly reloaded via the
[ReloadConfiguration](../keyassignment/ReloadConfiguration.md) key action, and
when [window:set_config_overrides](../window/set_config_overrides.md) is called
for the window.

This event is fire-and-forget from the perspective of wezterm; it fires the
event to advise of the config change, but has no other expectations.

If you call `window:set_config_overrides` from inside this event callback then
an additional `window-config-reloaded` event will be triggered.  You should
take care to avoid creating a loop by only calling
`window:set_config_overrides` when the actual override values are changed.

The first event parameter is a [`window` object](../window/index.md) that
represents the gui window.

The second event parameter is a [`pane` object](../pane/index.md) that
represents the active pane in that window.

```lua
local wezterm = require 'wezterm'

wezterm.on('window-config-reloaded', function(window, pane)
  wezterm.log_info 'the config was reloaded for this window!'
end)
```

