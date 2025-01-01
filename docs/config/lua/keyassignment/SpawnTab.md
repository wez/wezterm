# `SpawnTab`

Create a new tab in the current window.  The argument defines to which *domain* the tab belongs:

```lua
local act = wezterm.action

config.keys = {
  -- Create a new tab in the same domain as the current pane.
  -- This is usually what you want.
  {
    key = 't',
    mods = 'SHIFT|ALT',
    action = act.SpawnTab 'CurrentPaneDomain',
  },
  -- Create a new tab in the default domain
  { key = 't', mods = 'SHIFT|ALT', action = act.SpawnTab 'DefaultDomain' },
  -- Create a tab in a named domain
  {
    key = 't',
    mods = 'SHIFT|ALT',
    action = act.SpawnTab {
      DomainName = 'unix',
    },
  },
}
```

See [SpawnCommandInNewTab](SpawnCommandInNewTab.md) for a more flexible action
that allows more control over the spawned command.
