# `pane:is_alt_screen_active()`

{{since('20220807-113146-c2fee766')}}

Returns whether the alternate screen is active for the pane.

The alternate screen is a secondary screen that is activated by certain escape codes. The alternate screen has no scrollback, which makes it ideal for a "full-screen" terminal program like `vim` or `less` to do whatever they want on the screen without fear of destroying the user's scrollback. Those programs emit escape codes to return to the normal screen when they exit.
