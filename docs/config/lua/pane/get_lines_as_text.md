# `pane:get_lines_as_text([nlines])`

{{since('20201031-154415-9614e117')}}

Returns the textual representation (not including color or other attributes) of
the *physical* lines of text in the viewport as a string.

A *physical* line is a possibly-wrapped line that composes a row in the terminal
display matrix.  If you'd rather operate on *logical* lines, see
[pane:get_logical_lines_as_text](get_logical_lines_as_text.md).

If the optional `nlines` argument is specified then it is used to determine how
many lines of text should be retrieved.  The default (if `nlines` is not specified)
is to retrieve the number of lines in the viewport (the height of the pane).

The lines have trailing space removed from each line.  The lines will be
joined together in the returned string separated by a `\n` character.
Trailing blank lines are stripped, which may result in fewer lines being
returned than you might expect if the pane only had a couple of lines
of output.

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

See also: [pane:get_lines_as_escapes()](get_lines_as_escapes.md).
