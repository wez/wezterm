# `DetachDomain(domain)`

{{since('20220624-141144-bd1b7c5d')}}

Attempts to detach the specified domain.  Detaching a domain causes
it to disconnect and remove its set of windows, tabs and panes from
the local GUI.  Detaching does not cause those panes to close; if or
when you later attach to the domain, they'll still be there.

Not every domain supports detaching, and will log an error to the
error log/debug overlay.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

config.ssh_domains = {
  {
    name = 'devhost',
    remote_address = 'devhost.example.com',
  },
}
config.keys = {
  { key = 'U', mods = 'CTRL|SHIFT', action = act.AttachDomain 'devhost' },
  -- Detaches the domain associated with the current pane
  {
    key = 'D',
    mods = 'CTRL|SHIFT',
    action = act.DetachDomain 'CurrentPaneDomain',
  },
  -- Detaches the "devhost" domain
  {
    key = 'E',
    mods = 'CTRL|SHIFT',
    action = act.DetachDomain { DomainName = 'devhost' },
  },
}
```

See also: [AttachDomain](AttachDomain.md)
