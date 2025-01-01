---
title: wezterm.config_builder
tags:
 - utility
---

# wezterm.config_builder()

{{since('20230320-124340-559cb7b0')}}

Returns a config builder object that can be used to define your configuration:

```lua
local wezterm = require 'wezterm'

local config = wezterm.config_builder()

config.color_scheme = 'Batman'

return config
```

The config builder may look like a regular lua table but it is really a special
userdata type that knows how to log warnings or generate errors if you attempt
to define an invalid configuration option.

For example, with this erroneous config:

```lua
local wezterm = require 'wezterm'

-- Allow working with both the current release and the nightly
local config = {}
if wezterm.config_builder then
  config = wezterm.config_builder()
end

function helper(config)
  config.wrong = true
end

function another_layer(config)
  helper(config)
end

config.color_scheme = 'Batman'

another_layer(config)

return config
```

When evaluated by earlier versions of wezterm, this config will produce the
following warning, which is terse and doesn't provide any context on where the
mistake was made, requiring you to hunt around and find where `wrong` was
referenced:

```
11:44:11.668  WARN   wezterm_dynamic::error > `wrong` is not a valid Config field.  There are too many alternatives to list here; consult the documentation!
```

When using the config builder, the warning message is improved:

```
11:45:23.774  WARN   wezterm_dynamic::error > `wrong` is not a valid Config field.  There are too many alternatives to list here; consult the documentation!
11:45:23.787  WARN   config::lua            > Attempted to set invalid config option `wrong` at:
    [1] /tmp/wat.lua:10 global helper
    [2] /tmp/wat.lua:14 global another_layer
    [3] /tmp/wat.lua:19
```

The config builder provides a method that allows you to promote the warning to a lua error:

```
config:set_strict_mode(true)
```

The consequence of an error is that wezterm will show a configuration error
window and use the default config until you have resolved the error and
reloaded the configuration.  When not using strict mode, the warning
will not prevent the rest of your configuration from being used.



