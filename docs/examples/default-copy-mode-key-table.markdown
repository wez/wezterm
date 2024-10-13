```lua
local wezterm = require 'wezterm'
local act = wezterm.action

return {
  key_tables = {
    copy_mode = {
      { key = 'Tab', mods = 'NONE', action = act.CopyMode 'MoveForwardWord' },
      {
        key = 'Tab',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveBackwardWord',
      },
      {
        key = 'Enter',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfNextLine',
      },
      {
        key = 'Escape',
        mods = 'NONE',
        action = act.Multiple {
          { CopyMode = 'ScrollToBottom' },
          { CopyMode = 'Close' },
        },
      },
      {
        key = 'Space',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Cell' },
      },
      {
        key = '$',
        mods = 'NONE',
        action = act.CopyMode 'MoveToEndOfLineContent',
      },
      {
        key = '$',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToEndOfLineContent',
      },
      { key = ',', mods = 'NONE', action = act.CopyMode 'JumpReverse' },
      { key = '0', mods = 'NONE', action = act.CopyMode 'MoveToStartOfLine' },
      { key = ';', mods = 'NONE', action = act.CopyMode 'JumpAgain' },
      {
        key = 'F',
        mods = 'NONE',
        action = act.CopyMode { JumpBackward = { prev_char = false } },
      },
      {
        key = 'F',
        mods = 'SHIFT',
        action = act.CopyMode { JumpBackward = { prev_char = false } },
      },
      {
        key = 'G',
        mods = 'NONE',
        action = act.CopyMode 'MoveToScrollbackBottom',
      },
      {
        key = 'G',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToScrollbackBottom',
      },
      { key = 'H', mods = 'NONE', action = act.CopyMode 'MoveToViewportTop' },
      {
        key = 'H',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportTop',
      },
      {
        key = 'L',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportBottom',
      },
      {
        key = 'L',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportBottom',
      },
      {
        key = 'M',
        mods = 'NONE',
        action = act.CopyMode 'MoveToViewportMiddle',
      },
      {
        key = 'M',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToViewportMiddle',
      },
      {
        key = 'O',
        mods = 'NONE',
        action = act.CopyMode 'MoveToSelectionOtherEndHoriz',
      },
      {
        key = 'O',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToSelectionOtherEndHoriz',
      },
      {
        key = 'T',
        mods = 'NONE',
        action = act.CopyMode { JumpBackward = { prev_char = true } },
      },
      {
        key = 'T',
        mods = 'SHIFT',
        action = act.CopyMode { JumpBackward = { prev_char = true } },
      },
      {
        key = 'V',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Line' },
      },
      {
        key = 'V',
        mods = 'SHIFT',
        action = act.CopyMode { SetSelectionMode = 'Line' },
      },
      {
        key = '^',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },
      {
        key = '^',
        mods = 'SHIFT',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },
      { key = 'b', mods = 'NONE', action = act.CopyMode 'MoveBackwardWord' },
      { key = 'b', mods = 'ALT', action = act.CopyMode 'MoveBackwardWord' },
      { key = 'b', mods = 'CTRL', action = act.CopyMode 'PageUp' },
      {
        key = 'c',
        mods = 'CTRL',
        action = act.Multiple {
          { CopyMode = 'ScrollToBottom' },
          { CopyMode = 'Close' },
        },
      },
      {
        key = 'd',
        mods = 'CTRL',
        action = act.CopyMode { MoveByPage = 0.5 },
      },
      {
        key = 'e',
        mods = 'NONE',
        action = act.CopyMode 'MoveForwardWordEnd',
      },
      {
        key = 'f',
        mods = 'NONE',
        action = act.CopyMode { JumpForward = { prev_char = false } },
      },
      { key = 'f', mods = 'ALT', action = act.CopyMode 'MoveForwardWord' },
      { key = 'f', mods = 'CTRL', action = act.CopyMode 'PageDown' },
      {
        key = 'g',
        mods = 'NONE',
        action = act.CopyMode 'MoveToScrollbackTop',
      },
      {
        key = 'g',
        mods = 'CTRL',
        action = act.Multiple {
          { CopyMode = 'ScrollToBottom' },
          { CopyMode = 'Close' },
        },
      },
      { key = 'h', mods = 'NONE', action = act.CopyMode 'MoveLeft' },
      { key = 'j', mods = 'NONE', action = act.CopyMode 'MoveDown' },
      { key = 'k', mods = 'NONE', action = act.CopyMode 'MoveUp' },
      { key = 'l', mods = 'NONE', action = act.CopyMode 'MoveRight' },
      {
        key = 'm',
        mods = 'ALT',
        action = act.CopyMode 'MoveToStartOfLineContent',
      },
      {
        key = 'o',
        mods = 'NONE',
        action = act.CopyMode 'MoveToSelectionOtherEnd',
      },
      {
        key = 'q',
        mods = 'NONE',
        action = act.Multiple {
          { CopyMode = 'ScrollToBottom' },
          { CopyMode = 'Close' },
        },
      },
      {
        key = 't',
        mods = 'NONE',
        action = act.CopyMode { JumpForward = { prev_char = true } },
      },
      {
        key = 'u',
        mods = 'CTRL',
        action = act.CopyMode { MoveByPage = -0.5 },
      },
      {
        key = 'v',
        mods = 'NONE',
        action = act.CopyMode { SetSelectionMode = 'Cell' },
      },
      {
        key = 'v',
        mods = 'CTRL',
        action = act.CopyMode { SetSelectionMode = 'Block' },
      },
      { key = 'w', mods = 'NONE', action = act.CopyMode 'MoveForwardWord' },
      {
        key = 'y',
        mods = 'NONE',
        action = act.Multiple {
          { CopyTo = 'ClipboardAndPrimarySelection' },
          { CopyMode = 'ScrollToBottom' },
          { CopyMode = 'Close' },
        },
      },
      { key = 'PageUp', mods = 'NONE', action = act.CopyMode 'PageUp' },
      { key = 'PageDown', mods = 'NONE', action = act.CopyMode 'PageDown' },
      {
        key = 'End',
        mods = 'NONE',
        action = act.CopyMode 'MoveToEndOfLineContent',
      },
      {
        key = 'Home',
        mods = 'NONE',
        action = act.CopyMode 'MoveToStartOfLine',
      },
      { key = 'LeftArrow', mods = 'NONE', action = act.CopyMode 'MoveLeft' },
      {
        key = 'LeftArrow',
        mods = 'ALT',
        action = act.CopyMode 'MoveBackwardWord',
      },
      {
        key = 'RightArrow',
        mods = 'NONE',
        action = act.CopyMode 'MoveRight',
      },
      {
        key = 'RightArrow',
        mods = 'ALT',
        action = act.CopyMode 'MoveForwardWord',
      },
      { key = 'UpArrow', mods = 'NONE', action = act.CopyMode 'MoveUp' },
      { key = 'DownArrow', mods = 'NONE', action = act.CopyMode 'MoveDown' },
    },
  },
}
```
