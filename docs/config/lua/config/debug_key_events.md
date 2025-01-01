---
tags:
  - keys
  - debug
---
# `debug_key_events = false`

When set to true, each key event will be logged by the GUI layer as an INFO
level log message on the stderr stream from wezterm.  **You will typically need
to launch `wezterm` directly from another terminal to see this logging**.

This can be helpful in figuring out how keys are being decoded on your system,
or for discovering the system-dependent "raw" key code values.

```lua
config.debug_key_events = true
```

Produces logs like the following when typing `ls`: (artificially wrapped
to make these docs more readable):

```
 2021-02-20T17:04:28.149Z INFO  wezterm_gui::gui::termwindow   > key_event 
   KeyEvent { key: Char('l'), modifiers: NONE, raw_key: None,
   raw_modifiers: NONE, raw_code: Some(46), repeat_count: 1, key_is_down: true }
 2021-02-20T17:04:28.605Z INFO  wezterm_gui::gui::termwindow   > key_event
   KeyEvent { key: Char('s'), modifiers: NONE, raw_key: None, raw_modifiers: NONE,
   raw_code: Some(39), repeat_count: 1, key_is_down: true }
```

The key event has a number of fields:

* `key` is the decoded key after keymapping and composition effects.  For
  example `Char('l')` occurs when typing the `l` key and `Char('L')` occurs
  when doing the same but with `SHIFT` held down.  It could also be one
  of the keycode identifiers listed in
  [the Configuring Key Assignments](../../keys.md#configuring-key-assignments)
  section.
* `modifiers` indicates which modifiers are active after keymapping and composition
  effects.  For example, typing `l` with `SHIFT` held down produces
  `key: Char('L'), modifiers: NONE` because the `SHIFT` key composed to produce
  the uppercase `L`.
* `raw_key` represents the key press prior to any keymapping/composition events.
  If `raw_key` would be the same as `key` then `raw_key` will be printed as `NONE`.
* `raw_modifiers` represents the state of the modifier keys prior to any keymapping
  or composition effects.  For example, typeing `l` with `SHIFT` held down produces
  `raw_modifiers: SHIFT`.
* `raw_code` is the hardware-and-or-windowing-system-dependent raw keycode value
  associated with the key press.  This generally represents the physical location
  of the key independent of keymapping.
* `repeat_count` is usually `1` but on some systems may be larger number to
  indicate that the key is being held down and that the system is synthesizing
  multiple key-presses based on the system key repeat settings.
* `key_is_down` indicates whether the key is being pressed or released. This
  will always be true when debug logging, as logging and key press handling is
  only triggered on key press events in wezterm.

