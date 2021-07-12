# bypass_mouse_reporting_modifiers = "SHIFT"

*Since: nightly builds only*

If an application has enabled mouse reporting mode, mouse events are sent
directly to the application, and do not get routed through the mouse
assignment logic.

Holding down the `bypass_mouse_reporting_modifiers` modifier key(s) will
prevent the event from being passed to the application.

The default value for `bypass_mouse_reporting_modifiers` is `SHIFT`, which
means that holding down shift while clicking will not send the mouse
event to eg: vim running in mouse mode and will instead treat the event
as though `SHIFT` was not pressed and then match it against the mouse
assignments.

```lua
return {
  -- Use ALT instead of SHIFT to bypass application mouse reporting
  bypass_mouse_reporting_modifiers = "ALT"
}
```
