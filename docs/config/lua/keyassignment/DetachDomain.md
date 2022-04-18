# DetachDomain(domain)

*Since: nightly builds only*

Attempts to detach the specified domain.  Detaching a domain causes
it to disconnect and remove its set of windows, tabs and panes from
the local GUI.  Detaching does not cause those panes to close; if or
when you later attach to the domain, they'll still be there.

Not every domain supports detaching, and will log an error to the
error log/debug overlay.

```lua
local wezterm = require 'wezterm'

return {
  ssh_domains = {
    {
      name = "devhost",
      remote_address = "devhost.example.com",
    }
  },
  keys = {
    {key="U", mods="CTRL|SHIFT", action=wezterm.action{AttachDomain="devhost"}},
    -- Detaches the domain associated with the current pane
    {key="D", mods="CTRL|SHIFT", action=wezterm.action{DetachDomain="CurrentPaneDomain"}},
    -- Detaches the "devhost" domain
    {key="E", mods="CTRL|SHIFT", action=wezterm.action{DetachDomain={DomainName="devhost"}}},
  },
}

```
