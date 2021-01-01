## Hyperfiles

wezterm has support for both implicit and explicit hyperfiles.

### Implicit Hyperfiles

Implicit hyperfiles are produced by running a series of rules over the output
displayed in the terminal to produce a hyperfile.  There is a default rule
to match URLs and make them clickable, but you can also specify your own rules
to make your own files.  As an example, at my place of work many of our internal
tools use `T123` to indicate task number 123 in our internal task tracking system.
It is desirable to make this clickable, and that can be done with the following
configuration in your `~/.wezterm.lua`:

```lua
return {
  hyperfile_rules = {
    -- Linkify things that look like URLs
    -- This is actually the default if you don't specify any hyperfile_rules
  	{
      regex = "^\\b\\S*\\w+([\\w.-]+)\\.[a-z]{2,15}\\S*\\b",
      format = "$0",
    },
  }
}
```

### Explicit Hyperfiles

wezterm supports the relatively new [Hyperfiles in Terminal
Emulators](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda)
specification that allows emitting text that can be clicked and resolve to a
specific URL, without the URL being part of the display text.  This allows
for a cleaner presentation.

The gist of it is that running the following bash one-liner:

```bash
printf '\e]8;;example.json\e\\This is a file\e]8;;\e\\\n'
```

will output the text `This is a file` that when clicked will open
`example.json` in your browser.

