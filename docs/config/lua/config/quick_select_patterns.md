# quick_select_patterns

*Since: 20210502-130208-bff6815d*

Specify additional patterns to match when in [quick select mode](../../../quickselect.md).
This setting is a table listing out a set of regular expressions.

```lua
return {
  quick_select_patterns = {
    -- match things that look like sha1 hashes
    -- (this is actually one of the default patterns)
    "[0-9a-f]{7,40}",
  }
}
```

