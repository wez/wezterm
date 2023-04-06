# `InputSelector`

{{since('nightly')}}

Activates an overlay to display a list of choices for the user
to select from.

When the user accepts a line, emits an event that allows you to act
upon the input.

`InputSelector` accepts three fields:

* `title` - the title that will be set for the overlay pane
* `choices` - a lua table consisting of the potential choices. Each entry
  is itself a table with a `label` field and an optional `id` field.
  The label will be shown in the list, while the id can be a different
  string that is meaningful to your action. The label can be used together
  with [wezterm.format](../wezterm/format.md) to produce styled test.
* `action` - and event callback registerd via `wezterm.action_callback`.  The
  callback's function signature is `(window, pane, id, label)` where `window` and
  `pane` are the [Window](../window/index.md) and [Pane](../pane/index.md)
  objects from the current pane and window, and `id` and `label` hold the
  corresponding fields from the selected choice. Both will be `nil` if
  the overlay is cancelled without selecting anything.

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
      for n = 1, 10 do
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
        },
        pane
      )
    end),
  },
}

return config
```

See also [PromptInputLine](PromptInputLine.md).

