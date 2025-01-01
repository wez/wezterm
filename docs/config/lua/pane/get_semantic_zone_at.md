# `pane:get_semantic_zone_at(x, y)`

{{since('20230320-124340-559cb7b0')}}

Resolves the semantic zone that encapsulates the supplied *x* and *y* coordinates.

*x* is the cell column index, where 0 is the left-most column.
*y* is the stable row index.

Use [pane:get_dimensions()](get_dimensions.md) to
retrieve the currently valid stable index values for the top of scrollback and
top of viewport.

```lua
-- If you have shell integration configured, returns the zone around
-- the current cursor position
function get_zone_around_cursor(pane)
  local cursor = pane:get_cursor_position()
  -- using x-1 here because the cursor may be one cell outside the zone
  local zone = pane:get_semantic_zone_at(cursor.x - 1, cursor.y)
  if zone then
    return pane:get_text_from_semantic_zone(zone)
  end
  return nil
end
```

See [Shell Integration](../../../shell-integration.md) for more information
about semantic zones.

