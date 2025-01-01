# `pane:get_cursor_position()`

{{since('20201031-154415-9614e117')}}

Returns a lua representation of the `StableCursorPosition` struct
that identifies the cursor position, visibility and shape.

It has the following fields:

 * `x` the horizontal cell index
 * `y` the vertical stable row index
 * `shape` the `CursorShape` enum value
 * `visibility` the `CursorVisibility` enum value


