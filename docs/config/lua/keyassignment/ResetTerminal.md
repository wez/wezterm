# `ResetTerminal`

{{since('20221119-145034-49b9839f')}}

Sends the `RIS` "Reset to Initial State" escape sequence (`ESC-c`) to the
output side of the current pane, causing the terminal emulator to reset its
state.

This will reset tab stops, margins, modes, graphic rendition, palette, activate
the primary screen, erase the display and move the cursor to the home position.

