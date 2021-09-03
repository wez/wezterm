# window:get_appearance()

*Since: 20210814-124438-54e29167*

This method returns the appearance of the window environment.  The appearance
can be one of the following 4 values:

* `"Light"` - the normal appearance, with dark text on a light background
* `"Dark"` - "dark mode", with predominantly dark colors and probably a lighter, lower contrasting, text color on a dark background
* `"LightHighContrast"` - light mode but with high contrast colors
* `"DarkHighContrast"` - dark mode but with high contrast colors

wezterm currently doesn't know how to interrogate the appearance on Wayland
systems, and will always report `"Light"`.

On macOS, X11 and Windows systems, wezterm is able to detect when the
appearance has changed and will generate a
[window-config-reloaded](../window-events/window-config-reloaded.md) event for each
window.

This example configuration shows how you can have your color scheme
automatically adjust to the current appearance:

```lua
local wezterm = require 'wezterm'

function scheme_for_appearance(appearance)
  if appearance:find("Dark") then
    return "Builtin Solarized Dark"
  else
    return "Builtin Solarized Light"
  end
end

wezterm.on("window-config-reloaded", function(window, pane)
  local overrides = window:get_config_overrides() or {}
  local appearance = window:get_appearance()
  local scheme = scheme_for_appearance(appearance)
  if overrides.color_scheme ~= scheme then
    overrides.color_scheme = scheme
    window:set_config_overrides(overrides)
  end
end)

return {
}
```

### Wayland GNOME Appearance

The GNOME desktop environment provides the `gsettings` tool that can
inform us of the selected appearance even in a Wayland session. We can
substitute the call to `window:get_appearance` above with a call to the
following function, which takes advantage of this.

```lua
function query_appearance_gnome()
  local success, stdout = wezterm.run_child_process(
    {"gsettings", "get", "org.gnome.desktop.interface", "gtk-theme"}
  )
  -- lowercase and remove whitespace
  stdout = stdout:lower():gsub("%s+", "")
  local mapping = {
     highcontrast = "LightHighContrast",
     highcontrastinverse = "DarkHighContrast",
     adwaita = "Light",
     ["adwaita-dark"] = "Dark"
  }
  local appearance = mapping[stdout]
  if appearance then
     return appearance
  end
  if stdout:find("dark") then
     return "Dark"
  end
  return "Light"
end
```

Since Wezterm will not fire a `window-config-reloaded`
event on Wayland, you will instead need to listen on the
[update-right-status](../window-events/update-right-status.md) event,
which will essentially poll for the appearance continuously.

