---
tags:
  - font
---
### warn_about_missing_glyphs = true

{{since('20210502-130208-bff6815d')}}

When set to true, if a glyph cannot be found for a given codepoint, then
the configuration error window will be shown with a pointer to the font
configuration docs.

You can set `warn_about_missing_glyphs = false` to prevent the configuration
error window from being displayed.

The default is `warn_about_missing_glyphs = true`.

