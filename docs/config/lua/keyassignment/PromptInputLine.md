# `PromptInputLine`

{{since('20230408-112425-69ae8472')}}

Activates an overlay to display a prompt and request a line of input
from the user.

When the user enters the line, emits an event that allows you to act
upon the input.

`PromptInputLine` accepts four fields:

* `description` - the text to show at the top of the display area. You may
  embed escape sequences and/or use [wezterm.format](../wezterm/format.md).
* `action` - and event callback registered via `wezterm.action_callback`.  The
  callback's function signature is `(window, pane, line)` where `window` and
  `pane` are the [Window](../window/index.md) and [Pane](../pane/index.md)
  objects from the current pane and window, and `line` is the text that the
  user entered. `line` may be `nil` if they hit Escape without entering
  anything, or CTRL-C to cancel the input.
* `prompt` - the text to show as the prompt. You may embed escape sequences
  and/or use [wezterm.format](../wezterm/format.md).  Defaults to: `"> "`. {{since('nightly', inline=True)}}
* `initial_value` - optional.  If provided, the initial content of the input
  field will be set to this value.  The user may edit it prior to submitting
  the input. {{since('nightly', inline=True)}}

## Example of interactively renaming the current tab

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

local config = wezterm.config_builder()
config.keys = {
  {
    key = 'E',
    mods = 'CTRL|SHIFT',
    action = act.PromptInputLine {
      description = 'Enter new name for tab',
      initial_value = 'My Tab Name',
      action = wezterm.action_callback(function(window, pane, line)
        -- line will be `nil` if they hit escape without entering anything
        -- An empty string if they just hit enter
        -- Or the actual line of text they wrote
        if line then
          window:active_tab():set_title(line)
        end
      end),
    },
  },
}

return config
```

## Example of interactively picking a name and creating a new workspace

Similar to the above, but prompts for a name prior to creating
the workspace.

This example also shows the use of `wezterm.format` to emit colored text.

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

local config = wezterm.config_builder()
config.keys = {
  {
    key = 'N',
    mods = 'CTRL|SHIFT',
    action = act.PromptInputLine {
      description = wezterm.format {
        { Attribute = { Intensity = 'Bold' } },
        { Foreground = { AnsiColor = 'Fuchsia' } },
        { Text = 'Enter name for new workspace' },
      },
      action = wezterm.action_callback(function(window, pane, line)
        -- line will be `nil` if they hit escape without entering anything
        -- An empty string if they just hit enter
        -- Or the actual line of text they wrote
        if line then
          window:perform_action(
            act.SwitchToWorkspace {
              name = line,
            },
            pane
          )
        end
      end),
    },
  },
}

return config
```

See also [InputSelector](InputSelector.md).
