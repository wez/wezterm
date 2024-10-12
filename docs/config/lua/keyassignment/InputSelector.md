# `InputSelector`

{{since('20230408-112425-69ae8472')}}

Activates an overlay to display a list of choices for the user
to select from.

When the user accepts a line, emits an event that allows you to act
upon the input.

`InputSelector` accepts the following fields:

* `title` - the title that will be set for the overlay pane
* `choices` - a lua table consisting of the potential choices. Each entry
  is itself a table with a `label` field and an optional `id` field.
  The label will be shown in the list, while the id can be a different
  string that is meaningful to your action. The label can be used together
  with [wezterm.format](../wezterm/format.md) to produce styled text.
* `action` - and event callback registered via `wezterm.action_callback`.  The
  callback's function signature is `(window, pane, id, label)` where `window` and
  `pane` are the [Window](../window/index.md) and [Pane](../pane/index.md)
  objects from the current pane and window, and `id` and `label` hold the
  corresponding fields from the selected choice. Both will be `nil` if
  the overlay is cancelled without selecting anything.
* `fuzzy` - a boolean that defaults to `false`. If `true`, InputSelector will start
  in its fuzzy finding mode (this is equivalent to starting the InputSelector and
  pressing / in the default mode).

{{since('20240127-113634-bbcac864')}}

These additional fields are also available:

* `alphabet` - a string of unique characters. The characters in the string are used
  to calculate one or two click shortcuts that can be used to quickly choose from
  the InputSelector when in the default mode. Defaults to:
  `"1234567890abcdefghilmnopqrstuvwxyz"`. (Without j/k so they can be used for movement
  up and down.)
* `description` - a string to display when in the default mode. Defaults to:
  `"Select an item and press Enter = accept,  Esc = cancel,  / = filter"`.
* `fuzzy_description` - a string to display when in fuzzy finding mode. Defaults to:
  `"Fuzzy matching: "`.


### Key Assignments

The default key assignments in the InputSelector are as follows:

| Action  |  Key Assignment |
|---------|-------------------|
| Add to selection string until a match is found (if in the default mode) | Any key in `alphabet` {{since('20240127-113634-bbcac864', inline=True)}} |
| Select matching number (if in the default mode) | <kbd>1</kbd> to <kbd>9</kbd> {{since('20230408-112425-69ae8472', inline=True)}} |
| Start fuzzy search (if in the default mode) | <kbd>/</kbd> |
| Add to filtering string (if in fuzzy finding mode) | Any key not listed below |
| Remove from selection or filtering string | <kbd>Backspace</kbd> |
| Pick currently highlighted line | <kbd>Enter</kbd> |
|                                 | <kbd>LeftClick</kbd> (with mouse) |
| Move Down      | <kbd>DownArrow</kbd> |
|                | <kbd>Ctrl</kbd> + <kbd>N</kbd> |
|                | <kbd>Ctrl</kbd> + <kbd>J</kbd> {{since('20240127-113634-bbcac864', inline=True)}} |
|                | <kbd>j</kbd> (if not in `alphabet`) |
| Move Up        | <kbd>UpArrow</kbd>  |
|                | <kbd>Ctrl</kbd> + <kbd>P</kbd> |
|                | <kbd>Ctrl</kbd> + <kbd>K</kbd> {{since('20240127-113634-bbcac864', inline=True)}} |
|                | <kbd>k</kbd>  (if not in `alphabet`)   |
| Quit     | <kbd>Ctrl</kbd> + <kbd>G</kbd> |
|          | <kbd>Ctrl</kbd> + <kbd>C</kbd> {{since('20240127-113634-bbcac864', inline=True)}} |
|          | <kbd>Escape</kbd> |

Note: If the InputSelector is started with `fuzzy` set to `false`, then <kbd>Backspace</kbd> can go from fuzzy finding mode back to the default mode when pressed while the filtering string is empty.

## Example of choosing some canned text to enter into the terminal

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

config.keys = {
  {
    key = 'E',
    mods = 'CTRL|SHIFT',
    action = act.InputSelector {
      action = wezterm.action_callback(function(window, pane, id, label)
        if not id and not label then
          wezterm.log_info 'cancelled'
        else
          wezterm.log_info('you selected ', id, label)
          pane:send_text(id)
        end
      end),
      title = 'I am title',
      choices = {
        -- This is the first entry
        {
          -- Here we're using wezterm.format to color the text.
          -- You can just use a string directly if you don't want
          -- to control the colors
          label = wezterm.format {
            { Foreground = { AnsiColor = 'Red' } },
            { Text = 'No' },
            { Foreground = { AnsiColor = 'Green' } },
            { Text = ' thanks' },
          },
          -- This is the text that we'll send to the terminal when
          -- this entry is selected
          id = 'Regretfully, I decline this offer.',
        },
        -- This is the second entry
        {
          label = 'WTF?',
          id = 'An interesting idea, but I have some questions about it.',
        },
        -- This is the third entry
        {
          label = 'LGTM',
          id = 'This sounds like the right choice',
        },
      },
    },
  },
}

return config
```

## Example of dynamically constructing a list

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

config.keys = {
  {
    key = 'R',
    mods = 'CTRL|SHIFT',
    action = wezterm.action_callback(function(window, pane)
      -- We're going to dynamically construct the list and then
      -- show it.  Here we're just showing some numbers but you
      -- could read or compute data from other sources

      local choices = {}
      for n = 1, 20 do
        table.insert(choices, { label = tostring(n) })
      end

      window:perform_action(
        act.InputSelector {
          action = wezterm.action_callback(function(window, pane, id, label)
            if not id and not label then
              wezterm.log_info 'cancelled'
            else
              wezterm.log_info('you selected ', id, label)
              -- Since we didn't set an id in this example, we're
              -- sending the label
              pane:send_text(label)
            end
          end),
          title = 'I am title',
          choices = choices,
          alphabet = '123456789',
          description = 'Write the number you want to choose or press / to search.',
        },
        pane
      )
    end),
  },
}

return config
```

## Example of switching between a list of workspaces with the InputSelector

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = wezterm.config_builder()

config.keys = {
  {
    key = 'S',
    mods = 'CTRL|SHIFT',
    action = wezterm.action_callback(function(window, pane)
      -- Here you can dynamically construct a longer list if needed

      local home = wezterm.home_dir
      local workspaces = {
        { id = home, label = 'Home' },
        { id = home .. '/work', label = 'Work' },
        { id = home .. '/personal', label = 'Personal' },
        { id = home .. '/.config', label = 'Config' },
      }

      window:perform_action(
        act.InputSelector {
          action = wezterm.action_callback(
            function(inner_window, inner_pane, id, label)
              if not id and not label then
                wezterm.log_info 'cancelled'
              else
                wezterm.log_info('id = ' .. id)
                wezterm.log_info('label = ' .. label)
                inner_window:perform_action(
                  act.SwitchToWorkspace {
                    name = label,
                    spawn = {
                      label = 'Workspace: ' .. label,
                      cwd = id,
                    },
                  },
                  inner_pane
                )
              end
            end
          ),
          title = 'Choose Workspace',
          choices = workspaces,
          fuzzy = true,
          fuzzy_description = 'Fuzzy find and/or make a workspace',
        },
        pane
      )
    end),
  },
}

return config
```




See also [PromptInputLine](PromptInputLine.md).

