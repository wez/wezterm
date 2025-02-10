---
tags:
  - hyperlink
---
# `hyperlink_rules`

Defines rules to match text from the terminal output and generate
clickable links.

The value is a list of rule entries. Each entry has the following fields:

* `regex` - the regular expression to match (see supported [Regex syntax](https://docs.rs/regex/latest/regex/#syntax))
* `format` - Controls which parts of the regex match will be used to form the link.
  Must have a `prefix:` signaling the protocol type (e.g., `https:`/`mailto:`),
  which can either come from the regex match or needs to be explicitly added.
  The format string can use placeholders like `$0`, `$1`, `$2` etc. that will be replaced
  with that numbered capture group.  So, `$0` will take the entire
  region of text matched by the whole regex, while `$1` matches out
  the first capture group.  In the example below, `mailto:$0` is
  used to prefix a protocol to the text to make it into an URL.

{{since('20230320-124340-559cb7b0', outline=True)}}
    * `highlight` - specifies the range of the matched text that should be
      highlighted/underlined when the mouse hovers over the link.  The value is
      a number that corresponds to a capture group in the regex.  The default
      is `0`, highlighting the entire region of text matched by the regex.  `1`
      would be the first capture group, and so on.

{{since('20230408-112425-69ae8472', outline=True)}}
    The regex syntax now supports backreferences and look around assertions.
    See [Fancy Regex Syntax](https://docs.rs/fancy-regex/latest/fancy_regex/#syntax)
    for the extended syntax, which builds atop the underlying
    [Regex syntax](https://docs.rs/regex/latest/regex/#syntax).
    In prior versions, only the base
    [Regex syntax](https://docs.rs/regex/latest/regex/#syntax) was supported.

Assigning `hyperlink_rules` overrides the built-in default rules.

The default value for `hyperlink_rules` can be retrieved using
[wezterm.default_hyperlink_rules()](../wezterm/default_hyperlink_rules.md),
and is shown below:

```lua
config.hyperlink_rules = {
  -- Matches: a URL in parens: (URL)
  {
    regex = '\\((\\w+://\\S+)\\)',
    format = '$1',
    highlight = 1,
  },
  -- Matches: a URL in brackets: [URL]
  {
    regex = '\\[(\\w+://\\S+)\\]',
    format = '$1',
    highlight = 1,
  },
  -- Matches: a URL in curly braces: {URL}
  {
    regex = '\\{(\\w+://\\S+)\\}',
    format = '$1',
    highlight = 1,
  },
  -- Matches: a URL in angle brackets: <URL>
  {
    regex = '<(\\w+://\\S+)>',
    format = '$1',
    highlight = 1,
  },
  -- Then handle URLs not wrapped in brackets
  {
    regex = '\\b\\w+://\\S+[)/a-zA-Z0-9-]+',
    format = '$0',
  },
  -- implicit mailto link
  {
    regex = '\\b\\w+@[\\w-]+(\\.[\\w-]+)+\\b',
    format = 'mailto:$0',
  },
}
```

!!! note
    In quoted Lua string literals the backslash character must be
    quoted even if the following character isn't meaningful to Lua
    when quoted by a backslash. That means that you'll always want to
    double it up as `\\` when using it in a regex string.

    Alternatively, you can use the alternative string literal
    syntax; the following two examples are equivalent:

    ```lua
    regex = [[\b[tT](\d+)\b]]
    ```

    ```lua
    regex = '\\b[tT](\\d+)\\b'
    ```

Some other examples include:

```lua
-- Use the defaults as a base
config.hyperlink_rules = wezterm.default_hyperlink_rules()

-- make task numbers clickable
-- the first matched regex group is captured in $1.
table.insert(config.hyperlink_rules, {
  regex = [[\b[tt](\d+)\b]],
  format = 'https://example.com/tasks/?t=$1',
})

-- make username/project paths clickable. this implies paths like the following are for github.
-- ( "nvim-treesitter/nvim-treesitter" | wbthomason/packer.nvim | wezterm/wezterm | "wezterm/wezterm.git" )
-- as long as a full url hyperlink regex exists above this it should not match a full url to
-- github or gitlab / bitbucket (i.e. https://gitlab.com/user/project.git is still a whole clickable url)
table.insert(config.hyperlink_rules, {
  regex = [[["]?([\w\d]{1}[-\w\d]+)(/){1}([-\w\d\.]+)["]?]],
  format = 'https://www.github.com/$1/$3',
})
```
