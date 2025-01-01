---
tags:
  - appearance
  - text_cursor
---
# `force_reverse_video_cursor = false`

{{since('20210502-130208-bff6815d')}}

When `force_reverse_video_cursor = true`, override the `cursor_fg`,
`cursor_bg`, `cursor_border` settings from the color scheme and force the
cursor to use reverse video colors based on the `foreground` and `background`
colors.

When `force_reverse_video_cursor = false` (the default), `cursor_fg`,
`cursor_bg` and `cursor_border` color scheme settings are applied as normal.

{{since('20220319-142410-0fcdea07')}}

If escape sequences are used to change the cursor color, they will take
precedence over `force_reverse_video_cursor`.  In earlier releases, setting
`force_reverse_video_cursor = true` always ignored the configured cursor color.
