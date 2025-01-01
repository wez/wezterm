# ``ShowDebugOverlay``

{{since('20210814-124438-54e29167')}}

Overlays the current tab with the debug overlay, which is a combination
of a debug log and a lua [REPL](https://en.wikipedia.org/wiki/Read%E2%80%93eval%E2%80%93print_loop).

The REPL has the following globals available:

* `wezterm` - the [wezterm](../wezterm/index.md) module is pre-imported
* `window` - the [window](../window/index.md) object for the current window

The lua context in the REPL is not connected to any global state; you cannot use it
to dynamically assign event handlers for example.  It is primarily useful for
prototyping lua snippets before you integrate them fully into your config.

```lua
config.keys = {
  -- CTRL-SHIFT-l activates the debug overlay
  { key = 'L', mods = 'CTRL', action = wezterm.action.ShowDebugOverlay },
}
```
