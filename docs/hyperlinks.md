wezterm has support for both implicit and explicit hyperlinks.

### Implicit Hyperlinks

Implicit hyperlinks are produced by running a series of rules over the output
displayed in the terminal to produce a hyperlink.  There is a default rule
to match URLs and make them clickable, but you can also specify your own rules
to make your own links.

As an example, at my place of work many of our internal tools use `T123` to
indicate task number 123 in our internal task tracking system.  It is desirable
to make this clickable, and that can be done with the following configuration
in your `~/.wezterm.lua`:

```lua
local wezterm = require 'wezterm'
local config = {}

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

return config
```

See also [hyperlink_rules](config/lua/config/hyperlink_rules.md) and
[default_hyperlink_rules](config/lua/wezterm/default_hyperlink_rules.md).


### Explicit Hyperlinks

wezterm supports the relatively new [Hyperlinks in Terminal
Emulators](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
specification that allows emitting text that can be clicked and resolve to a
specific URL, without the URL being part of the display text.  This allows
for a cleaner presentation.

The gist of it is that running the following bash one-liner:

```bash
printf '\e]8;;http://example.com\e\\This is a link\e]8;;\e\\\n'
```

will output the text `This is a link` that when clicked will open
`http://example.com` in your browser.
