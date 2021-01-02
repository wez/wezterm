## Hyperfiles

wezterm supports the relatively new Hyperfiles
to Click open files in code editor or local program.

### Implicit Hyperfiles

Implicit hyperfiles are produced by running a series of rules over the output
displayed in the terminal to produce a hyperfile. 
It is desirable to make this clickable, and that can be done with the following
configuration in your `~/.wezterm.lua`:

```lua
return {
  hyperlink_rules = {
  	-- Hyperfile:line
  	{
      regex = "^\\s*[a-zA-Z0-9/_\\-\\. ]+\\.?[a-zA-Z0-9]+:[0-9]+",
      format = "hyperfile:$0"
    },
    -- Hyperfile Diff in ... at line
  	{
  	  regex = "Diff in [a-zA-Z0-9/_\\-\\. ]+\\.?[a-zA-Z0-9]+",
      format = "hyperfile:$0"
    }
  }
}
```

### Explicit Hyperfiles


```bash
printf '\e]88;;example.json:10\e\\This is a file\e]88;;\e\\\n'
```

will output the text `This is a file` that when clicked will open
`example.json` in your browser.

