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
### Wayland GNOME Workaround

If you happen to be running a Wayland session through GNOME desktop,
a workaround exists in which you can query the `gsettings` command
for the theme name. While GNOME does not specifically support
appearance, it is common for themes to provide a dark variant (and
this is the case for the default "Adwaita" theme), which are typically
named with "-dark" appended to the theme name. With this heuristic,
we can determine if GNOME is in dark mode or not, and abuse the
[update-right-status](../window-events/update-right-status.md)
event to poll for changes. When coupled with the [Night
Theme](https://nightthemeswitcher.romainvigier.fr/) extension or
similar, you can enjoy a Wezterm that changes theme based on day night
cycle with the rest of your system.

```lua
function·compute_scheme_gnome_wayland()¬
··local·success,·stdout·=·wezterm.run_child_process(¬
····{"gsettings",·"get",·"org.gnome.desktop.interface",·"gtk-theme"}¬
··)¬
··if·stdout:match("-dark")·then¬
····return·"Builtin Solarized Dark"¬
··else¬
····return·"Builtin Solarized Light"¬
··end¬
end¬
¬
wezterm.on("update-right-status",·function(window,·pane)¬
··local·overrides·=·window:get_config_overrides()·or·{}¬
··local·scheme·=·compute_scheme_gnome_wayland()¬
··if·overrides.color_scheme·~=·scheme·then¬
····overrides.color_scheme·=·scheme¬
····window:set_config_overrides(overrides)¬
··end¬
end)¬
```

