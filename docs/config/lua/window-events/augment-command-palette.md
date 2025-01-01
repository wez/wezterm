# `augment-command-palette`

{{since('20230712-072601-f4abf8fd')}}

This event is emitted when the [Command Palette](../keyassignment/ActivateCommandPalette.md) is shown.

It's purpose is to enable you to add additional entries to the list of commands
shown in the palette.

This hook is synchronous; calling asynchronous functions will not succeed.

The return value is a table listing the additional entries.  Each element of the
returned table may have the following fields:

* `brief` - required: the brief description for the entry
* `doc` - optional: a long description that may be shown after the entry, or that
  may be used in future versions of wezterm to provide more information about the
  command.
* `action` - the action to take when the item is activated. Can be any key assignment
  action.
* `icon` - optional Nerd Fonts glyph name to use for the icon for the entry. See
  [wezterm.nerdfonts](../wezterm/nerdfonts.md) for a list of icon names.

## Adding a Rename Tab entry to the palette

In this example, an entry is added for renaming tabs:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action

local config = wezterm.config_builder()

wezterm.on('augment-command-palette', function(window, pane)
  return {
    {
      brief = 'Rename tab',
      icon = 'md_rename_box',

      action = act.PromptInputLine {
        description = 'Enter new name for tab',
        initial_value = 'My Tab Name',
        action = wezterm.action_callback(function(window, pane, line)
          if line then
            window:active_tab():set_title(line)
          end
        end),
      },
    },
  }
end)

return config
```
