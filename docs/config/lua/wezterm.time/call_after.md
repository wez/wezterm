# `wezterm.time.call_after(interval_seconds, function)`

{{since('20220807-113146-c2fee766')}}

Arranges to call your callback function after the specified number of seconds
have elapsed.

Here's a contrived example that demonstrates a configuration that
varies based on the time. In this case, the idea is that the background
color is derived from the current number of minutes past the hour.

In order for the value to be picked up for the next minute, `call_after`
is used to schedule a callback 60 seconds later and it then generates
a background color by extracting the current minute value and scaing
it to the range 0-255 and using that to assign a background color:

```lua
local wezterm = require 'wezterm'

-- Reload the configuration every minute
wezterm.time.call_after(60, function()
  wezterm.reload_configuration()
end)

local amount =
  math.ceil((tonumber(wezterm.time.now():format '%M') / 60) * 255)

return {
  colors = {
    background = 'rgb(' .. amount .. ',' .. amount .. ',' .. amount .. ')',
  },
}
```

With great power comes great responsibility: if you schedule a lot of frequent
callbacks, or frequently reload your configuration in this way, you may
increase the CPU load on your system because you are asking it to work harder.

{{since('20230320-124340-559cb7b0')}}

You can use fractional seconds to delay by more precise intervals.
