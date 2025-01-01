{{since('20220408-101518-b908e2dd')}}

In addition to the default key table defined by the `keys` configuration
option, `wezterm` supports defining additional named key tables using the
`key_tables` configuration option.

On its own, a named table doesn't do anything, but when paired with the
`ActivateKeyTable` action, some powerful keyboard customization is possible.

As a motivating example, let's consider working with panes. In the default
config `CTRL+SHIFT+ArrowKey` will activate a pane in the direction of the arrow
key, while `CTRL+SHIFT+ALT+ArrowKey` will resize a pane in the direction of the
arrow key.  Our goal is to avoid holding down so many keys at once, or even
having to remember so many key combinations, so what we'd like to do is use
`CTRL-SHIFT-SPACE` as a leader prefix to select between resize and activation
modes, using `r` for resize and `a` for activation:

```lua
local wezterm = require 'wezterm'
local act = wezterm.action
local config = {}

-- Show which key table is active in the status area
wezterm.on('update-right-status', function(window, pane)
  local name = window:active_key_table()
  if name then
    name = 'TABLE: ' .. name
  end
  window:set_right_status(name or '')
end)

config.leader = { key = 'Space', mods = 'CTRL|SHIFT' }
config.keys = {
  -- CTRL+SHIFT+Space, followed by 'r' will put us in resize-pane
  -- mode until we cancel that mode.
  {
    key = 'r',
    mods = 'LEADER',
    action = act.ActivateKeyTable {
      name = 'resize_pane',
      one_shot = false,
    },
  },

  -- CTRL+SHIFT+Space, followed by 'a' will put us in activate-pane
  -- mode until we press some other key or until 1 second (1000ms)
  -- of time elapses
  {
    key = 'a',
    mods = 'LEADER',
    action = act.ActivateKeyTable {
      name = 'activate_pane',
      timeout_milliseconds = 1000,
    },
  },
}

config.key_tables = {
  -- Defines the keys that are active in our resize-pane mode.
  -- Since we're likely to want to make multiple adjustments,
  -- we made the activation one_shot=false. We therefore need
  -- to define a key assignment for getting out of this mode.
  -- 'resize_pane' here corresponds to the name="resize_pane" in
  -- the key assignments above.
  resize_pane = {
    { key = 'LeftArrow', action = act.AdjustPaneSize { 'Left', 1 } },
    { key = 'h', action = act.AdjustPaneSize { 'Left', 1 } },

    { key = 'RightArrow', action = act.AdjustPaneSize { 'Right', 1 } },
    { key = 'l', action = act.AdjustPaneSize { 'Right', 1 } },

    { key = 'UpArrow', action = act.AdjustPaneSize { 'Up', 1 } },
    { key = 'k', action = act.AdjustPaneSize { 'Up', 1 } },

    { key = 'DownArrow', action = act.AdjustPaneSize { 'Down', 1 } },
    { key = 'j', action = act.AdjustPaneSize { 'Down', 1 } },

    -- Cancel the mode by pressing escape
    { key = 'Escape', action = 'PopKeyTable' },
  },

  -- Defines the keys that are active in our activate-pane mode.
  -- 'activate_pane' here corresponds to the name="activate_pane" in
  -- the key assignments above.
  activate_pane = {
    { key = 'LeftArrow', action = act.ActivatePaneDirection 'Left' },
    { key = 'h', action = act.ActivatePaneDirection 'Left' },

    { key = 'RightArrow', action = act.ActivatePaneDirection 'Right' },
    { key = 'l', action = act.ActivatePaneDirection 'Right' },

    { key = 'UpArrow', action = act.ActivatePaneDirection 'Up' },
    { key = 'k', action = act.ActivatePaneDirection 'Up' },

    { key = 'DownArrow', action = act.ActivatePaneDirection 'Down' },
    { key = 'j', action = act.ActivatePaneDirection 'Down' },
  },
}

return config
```

### Key Table Activation Stack

Each `wezterm` GUI window maintains a stack of activations, which allows you to
create complex layering of keyboard customization.

The [ActivateKeyTable](lua/keyassignment/ActivateKeyTable.md) action will push
an entry to the stack, and provides `one_shot` and `timeout_milliseconds`
fields to affect when/how it will pop itself from the stack, and
`replace_current` to implicitly pop the current entry from the stack.

The [PopKeyTable](lua/keyassignment/PopKeyTable.md) action will explicitly pop
an entry from the stack.

The [ClearKeyTableStack](lua/keyassignment/ClearKeyTableStack.md) action will
clear the entire stack.

The stack is also cleared when the configuration is reloaded, so if you're
working on a complex key table setup and get stuck, you may be able to unstick
yourself by re-saving your wezterm configuration to trigger a reload.

{{since('20220624-141144-bd1b7c5d')}}

When resolving a key assignment, the top of stack is first searched for a match,
and if one is not found, the next entry on the stack is searched and so on until a match is found.

In previous releases, only a single lookup was performed on the top of the stack.

The new behavior allows key table activations to effectively layer over the top
of previously activated key assignments, making it a bit easier to compose key
assignments.

