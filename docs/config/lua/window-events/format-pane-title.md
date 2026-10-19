# `format-pane-title`

{{since('nightly')}}

The `format-pane-title` event is emitted when the text for a pane title bar
needs to be recomputed.  Pane title bars are enabled by setting
[pane_border_status](../config/pane_border_status.md) to `"Top"` or
`"Bottom"`.

This event is *synchronous* and must return as quickly as possible in order
to avoid blocking the GUI thread.  Avoid calling asynchronous functions such
as [wezterm.run_child_process](../wezterm/run_child_process.md) from inside
the handler.

The parameters to the event are:

* `pane` - a [PaneInformation](../PaneInformation.md) for the pane whose title is being computed
* `panes` - an array of [PaneInformation](../PaneInformation.md) for each pane in the active tab
* `tabs` - an array of [TabInformation](../TabInformation.md) for each tab in the window
* `config` - the effective configuration for the window
* `hover` - true if the mouse is hovering over the pane title bar
* `focused` - true if this pane is the currently focused pane

The return value must be a string.

If the event is not handled, or returns something other than a string, the
pane's window title (as set by the running program) is used instead.

Only the first `format-pane-title` event handler registered with
`wezterm.on("format-pane-title", ...)` will be executed.

## Basic example

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

config.pane_border_status = 'Top'

wezterm.on('format-pane-title', function(pane, panes, tabs, config, hover, focused)
  local indicator = focused and '● ' or '○ '
  local cwd = ''
  if pane.current_working_dir then
    local path = pane.current_working_dir.file_path or tostring(pane.current_working_dir)
    cwd = path:match('([^/]+)/?$') or path
  end
  return indicator .. cwd
end)

return config
```

## Persistent status messages via user variables

Because the pane title bar is refreshed independently of the shell prompt, it
is a natural place to display status information that should remain visible
while a long-running command executes.  Shell precmd/preexec hooks can push
status into the title bar by setting a [user variable](../pane/get_user_vars.md)
via an OSC 1337 escape sequence.

The value persists in the title bar until explicitly cleared, even as the
shell redraws its prompt repeatedly.

### Shell integration

Add a helper to your shell's init file that encodes and sends the variable:

```bash
# ~/.zshrc or ~/.bashrc
_wezterm_set_pane_status() {
  # Requires base64; no-op outside WezTerm
  [[ -z "$WEZTERM_PANE" ]] && return
  printf "\033]1337;SetUserVar=%s=%s\007" "$1" "$(printf '%s' "$2" | base64)"
}

# Show the current task in the pane title bar
_wezterm_set_pane_status STATUS "building…"
# Clear it when the task finishes
_wezterm_set_pane_status STATUS ""
```

With `precmd` / `PROMPT_COMMAND` integration you can automatically clear the
status when the prompt returns:

```bash
# Zsh
precmd() {
  _wezterm_set_pane_status STATUS ""
}

preexec() {
  # Show the command being run
  _wezterm_set_pane_status STATUS "$1"
}
```

### Lua handler

Read the user variable in `format-pane-title` and display it when set:

```lua
wezterm.on('format-pane-title', function(pane, panes, tabs, config, hover, focused)
  local indicator = focused and '● ' or '○ '

  -- Persistent status set by the shell via OSC 1337 SetUserVar
  local status = (pane.user_vars or {}).STATUS or ''
  if status ~= '' then
    return indicator .. status
  end

  -- Fall back to current working directory
  local cwd = pane.title or ''
  if pane.current_working_dir then
    local path = pane.current_working_dir.file_path or tostring(pane.current_working_dir)
    cwd = path:match('([^/]+)/?$') or path
  end
  return indicator .. cwd
end)
```

The `user-var-changed` event fires whenever a variable is set, which
immediately triggers a title bar refresh — there is no polling.

## See also

* [pane_border_status](../config/pane_border_status.md)
* [PaneInformation](../PaneInformation.md)
* [pane:get_user_vars()](../pane/get_user_vars.md)
* [user-var-changed](user-var-changed.md)
* [format-tab-title](format-tab-title.md)
