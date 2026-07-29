---
tags:
  - prompt
---

# `Confirmation`

{{since('nightly')}}

Activates an overlay to display a confirmation menu

When the user accepts a line, emits an event that allows you to act
upon the input.

`Confirmation` accepts the following fields:

* `message` - the text to show for confirmation. You may embed
  escape sequences and/or use [wezterm.format](../wezterm/format.md).
  Defaults to: `"🛑 Really continue?"`.
* `action` - event callback registered via `wezterm.action_callback`.  The
  callback's function signature is `(window, pane)` where `window` and
  `pane` are the [Window](../window/index.md) and [Pane](../pane/index.md)
  objects from the current pane and window. This callback is called when the
  user selects `Yes`.
* `cancel` - event callback registered via `wezterm.action_callback`.  The
  callback's function signature is `(window, pane)` where `window` and
  `pane` are the [Window](../window/index.md) and [Pane](../pane/index.md).
  This is an optional argument. If present, this callback is called when the
  user selects `No` or closes the confirmation menu.

## Example of choosing a program with user confirmation

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

config.keys = {
  {
    key = 'E',
    mods = 'CTRL|SHIFT',
    action = act.Confirmation {
      message = 'Do you want to run htop in a new window?',
      action = wezterm.action_callback(function(window, pane)
        window:perform_action(
          act.SpawnCommandInNewWindow { args = { 'htop' } },
          pane
        )
      end),
      cancel = wezterm.action_callback(function(window, pane)
        wezterm.log_error 'user declined'
      end),
    },
  },
}

return config
```




See also:
   * [InputSelector](InputSelector.md).
   * [PromptInputLine](PromptInputLine.md).
