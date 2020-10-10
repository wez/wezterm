# SpawnTab

Create a new tab in the current window.  The argument defines to which *domain* the tab belongs:

```lua
local wezterm = require 'wezterm';
return {
  keys = {
    -- Create a new tab in the same domain as the current pane.
    -- This is usually what you want.
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab="CurrentPaneDomain"}},
    -- Create a new tab in the default domain
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab="DefaultDomain"}},
    -- Create a tab in a named domain
    {key="t", mods="SHIFT|ALT", action=wezterm.action{SpawnTab={DomainName="unix"}}},
  }
}
```


