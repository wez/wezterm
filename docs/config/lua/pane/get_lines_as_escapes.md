# `pane:get_lines_as_escapes([nlines])`

{{since('20240127-113634-bbcac864')}}

Returns the textual representation (*including* color and other attributes) of
the *physical* lines of text in the viewport as a string with embedded ANSI
escape sequences to preserve the color and style of the text.

A *physical* line is a possibly-wrapped line that composes a row in the terminal
display matrix.

If the optional `nlines` argument is specified then it is used to determine how
many lines of text should be retrieved.  The default (if `nlines` is not specified)
is to retrieve the number of lines in the viewport (the height of the pane).

To obtain the entire scrollback, you can do something like this:

```lua
pane:get_lines_as_escapes(pane:get_dimensions().scrollback_rows)
```

## Example: opening scrollback in a pager

```lua
local wezterm = require 'wezterm'
local io = require 'io'
local os = require 'os'
local act = wezterm.action

wezterm.on('trigger-less-with-scrollback', function(window, pane)
  -- Retrieve the current pane's text
  local text =
    pane:get_lines_as_escapes(pane:get_dimensions().scrollback_rows)

  -- Create a temporary file to pass to the pager
  local name = os.tmpname()
  local f = io.open(name, 'w+')
  f:write(text)
  f:flush()
  f:close()

  -- Open a new window running less and tell it to open the file
  window:perform_action(
    act.SpawnCommandInNewWindow {
      args = { 'less', '-fr', name },
    },
    pane
  )

  -- Wait "enough" time for less to read the file before we remove it.
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
      action = act.EmitEvent 'trigger-less-with-scrollback',
    },
  },
}
```

See also: [pane:get_lines_as_text()](get_lines_as_text.md).
