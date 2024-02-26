---
tags:
  - quick_select
---
# `quick_select_patterns`

{{since('20210502-130208-bff6815d')}}

Specify additional patterns to match when in [quick select mode](../../../quickselect.md).
This setting is a table listing out a set of regular expressions.

```lua
config.quick_select_patterns = {
  -- match things that look like sha1 hashes
  -- (this is actually one of the default patterns)
  '[0-9a-f]{7,40}',
}
```

**Note:** usage of capturing groups is not recommented in most scenarios as all the patterns get combined into a big regex, where each of the pattern is an alternative in a capturing group.
Then capturing groups are used for matching logic. So you want to use non-capturing groups (`(?:)`) or lookaheads/lookbehinds instead.

{{since('20230408-112425-69ae8472', outline=True)}}
    The regex syntax now supports backreferences and look around assertions.
    See [Fancy Regex Syntax](https://docs.rs/fancy-regex/latest/fancy_regex/#syntax)
    for the extended syntax, which builds atop the underlying
    [Regex syntax](https://docs.rs/regex/latest/regex/#syntax).
    In prior versions, only the base
    [Regex syntax](https://docs.rs/regex/latest/regex/#syntax) was supported.

    This example matches the string `"bar"`, but only when not part of the string
    `"foo:bar"`:

    ```lua
    config.quick_select_patterns = {
        "(?<!foo:)bar"
    }
    ```
