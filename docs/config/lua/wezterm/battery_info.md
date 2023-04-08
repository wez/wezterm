---
title: wezterm.battery_info
tags:
 - utility
---

# `wezterm.battery_info()`

{{since('20210314-114017-04b7cedd')}}

This function returns battery information for each of the installed
batteries on the system.  This is useful for example to assemble
status information for the status bar.

The return value is an array of objects with the following fields:

* `state_of_charge` - the battery level expressed as a number between `0.0` (empty) and `1.0` (full)
* `vendor` - battery manufacturer name, or `"unknown"` if not known.
* `model` - the battery model string, or `"unknown"` if not known.
* `serial` - the battery serial number, or `"unknown"` if not known.
* `time_to_full` - if charging, how long until the battery is full (in seconds). May be `nil`.
* `time_to_empty` - if discharing, how long until the battery is empty (in seconds). May be `nil`.
* `state` - `"Charging"`, `"Discharging"`, `"Empty"`, `"Full"`, `"Unknown"`

This example shows the battery status for each battery, along with the date and time in the status bar:

```lua
local wezterm = require 'wezterm'

wezterm.on('update-right-status', function(window, pane)
  -- "Wed Mar 3 08:14"
  local date = wezterm.strftime '%a %b %-d %H:%M '

  local bat = ''
  for _, b in ipairs(wezterm.battery_info()) do
    bat = '🔋 ' .. string.format('%.0f%%', b.state_of_charge * 100)
  end

  window:set_right_status(wezterm.format {
    { Text = bat .. '   ' .. date },
  })
end)
```
