# ActivateCommandPalette

*Since: nightly builds only*

Activates the Command Palette, a modal overlay that enables discovery and activation of various commands.

```lua
return {
  keys = {
    {
      key = 'P',
      mods = 'CTRL',
      action = wezterm.action.ActivateCommandPalette,
    },
  },
}
```

<kbd>CTRL</kbd> + <kbd>SHIFT</kbd> + <kbd>P</kbd> is the default key assignment for `ActivateCommandPalette`.

The command palette shows a list of possible actions ranked by
[frecency](https://en.wikipedia.org/wiki/Frecency) of use from the command
palette.

<img src="../../../screenshots/command-palette.png">

### Key Assignments

| Action | Key Assignment |
|--------|----------------|
|Exit command palette| <kbd>Esc</kbd> |
|Highlight previous item| <kbd>UpArrow</kbd> |
|Highlight next item| <kbd>DownArrow</kbd> |
|Clear the selection| <kbd>CTRL</kbd> + <kbd>u</kbd> |
|Activate the selection| <kbd>Enter</kbd> |

Typing text (and using <kbd>Backspace</kbd>) allows you to fuzzy match possible
actions. Each keystroke will reduce the list of candidate actions to those that
fuzzy match, ranked in decreasing order of the match score.

Activating the selected item will close the command palette and then invoke the
action.
