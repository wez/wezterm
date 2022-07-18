## Hyperlinks

wezterm has support for both implicit and explicit hyperlinks.

### Implicit Hyperlinks

Implicit hyperlinks are produced by running a series of rules over the output
displayed in the terminal to produce a hyperlink.  There is a default rule
to match URLs and make them clickable, but you can also specify your own rules
to make your own links.  As an example, at my place of work many of our internal
tools use `T123` to indicate task number 123 in our internal task tracking system.
It is desirable to make this clickable, and that can be done with the following
configuration in your `~/.wezterm.lua`:

```lua
return {
  hyperlink_rules = {
    -- Linkify things that look like URLs and the host has a TLD name.
    -- Compiled-in default. Used if you don't specify any hyperlink_rules.
    {
      regex = '\\b\\w+://[\\w.-]+\\.[a-z]{2,15}\\S*\\b',
      format = '$0',
    },

    -- linkify email addresses
    -- Compiled-in default. Used if you don't specify any hyperlink_rules.
    {
      regex = [[\b\w+@[\w-]+(\.[\w-]+)+\b]],
      format = 'mailto:$0',
    },

    -- file:// URI
    -- Compiled-in default. Used if you don't specify any hyperlink_rules.
    {
      regex = [[\bfile://\S*\b]],
      format = '$0',
    },

    -- Linkify things that look like URLs with numeric addresses as hosts.
    -- E.g. http://127.0.0.1:8000 for a local development server,
    -- or http://192.168.1.1 for the web interface of many routers.
    {
      regex = [[\b\w+://(?:[\d]{1,3}\.){3}[\d]{1,3}\S*\b]],
      format = '$0',
    },

    -- Make task numbers clickable
    -- The first matched regex group is captured in $1.
    {
      regex = [[\b[tT](\d+)\b]],
      format = 'https://example.com/tasks/?t=$1',
    },

    -- Make username/project paths clickable. This implies paths like the following are for GitHub.
    -- ( "nvim-treesitter/nvim-treesitter" | wbthomason/packer.nvim | wez/wezterm | "wez/wezterm.git" )
    -- As long as a full URL hyperlink regex exists above this it should not match a full URL to
    -- GitHub or GitLab / BitBucket (i.e. https://gitlab.com/user/project.git is still a whole clickable URL)
    {
      regex = [[["]?([\w\d]{1}[-\w\d]+)(/){1}([-\w\d\.]+)["]?]],
      format = 'https://www.github.com/$1/$3',
    },
  },
}
```

Note that it is generally convenient to use literal strings (`[[...]]`)
when declaring your hyperlink rules, so you won't have to escape
backslashes.  In the example above, all cases except the first use
literal strings for their regular expressions.


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
