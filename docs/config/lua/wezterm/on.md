---
title: wezterm.on
tags:
 - utility
 - event
---
# `wezterm.on(event_name, callback)`

{{since('20201031-154415-9614e117')}}

This function follows the html/javascript naming for defining event handlers.

`wezterm.on` causes your specified `callback` to be called when `event_name`
is emitted.  Events can be emitted by wezterm itself, or through code/configuration
that you specify.

`wezterm.on` can register multiple callbacks for the same event; internally
an ordered list of callbacks is maintained for each event.  When the event
is emitted, each of the registered callbacks is called in the order that
they were registered.

The callback will receive the following parameters:
- a [`window` object](../window/index.md) that represents the active gui window.
- a [`pane` object](../pane/index.md) that represents the active pane.

If a callback returns `false` it will prevent any callbacks that were registered
after it from being triggered for the current event.  Some events have
a defined default action; returning `false` will prevent that default action
from being taken for the current event.

There is no way to de-register an event handler.  However, since the Lua
state is built from scratch when the configuration is reloaded, simply
reloading the configuration will clear any existing event handlers.

See [wezterm.action_callback](./action_callback.md) for a helper to define a custom action callback.

## Predefined Events

See [Window Events](../window-events/index.md) for a list of pre-defined
events.

## Custom Events

You may register handlers for arbitrary events for which wezterm itself
has no special knowledge.  It is recommended that you avoid event names
that are likely to be used future versions of wezterm in order to avoid
unexpected behavior if/when those names might be used in future.

The [wezterm.emit](emit.md) function and the [EmitEvent](../keyassignment/EmitEvent.md) key assignment can be used to
emit events.

# Example: opening whole scrollback in vim

In the following example, a key is assigned to capture the entire scrollback
and visible area of the active pane, write it to a file and then open that file
in the `vim` editor:

```lua
local wezterm = require 'wezterm'
local io = require 'io'
local os = require 'os'
local act = wezterm.action

wezterm.on('trigger-vim-with-scrollback', function(window, pane)
  -- Retrieve the text from the pane
  local text = pane:get_lines_as_text(pane:get_dimensions().scrollback_rows)

  -- Create a temporary file to pass to vim
  local name = os.tmpname()
  local f = io.open(name, 'w+')
  f:write(text)
  f:flush()
  f:close()

  -- Open a new window running vim and tell it to open the file
  window:perform_action(
    act.SpawnCommandInNewWindow {
      args = { 'vim', name },
    },
    pane
  )

  -- Wait "enough" time for vim to read the file before we remove it.
  -- The window creation and process spawn are asynchronous wrt. running
  -- this script and are not awaitable, so we just pick a number.
  --
  -- Note: We don't strictly need to remove this file, but it is nice
  -- to avoid cluttering up the temporary directory.
  wezterm.sleep_ms(1000)
  os.remove(name)
end)

return {
  keys = {
    {
      key = 'E',
      mods = 'CTRL',
      action = act.EmitEvent 'trigger-vim-with-scrollback',
    },
  },
}
```
