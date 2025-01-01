# WslDomain

{{since('20220319-142410-0fcdea07')}}

The `WslDomain` struct specifies information about an individual `WslDomain`,
which is used to tell wezterm how to interact with one of your locally
installed [WSL](https://docs.microsoft.com/en-us/windows/wsl/about)
distributions.

By mapping a distribution to a multiplexing domain, wezterm is better able to
support creating new tabs and panes with the same working directory as an
existing tab/pane running in that same domain.

By default, wezterm creates a list of `WslDomain` objects based on parsing the
output from `wsl -l -v` and assigns that as the value of the
[wsl_domains](config/wsl_domains.md) configuration option.

A `WslDomain` is a lua object with the following fields:

```lua
config.wsl_domains = {
  {
    -- The name of this specific domain.  Must be unique amonst all types
    -- of domain in the configuration file.
    name = 'WSL:Ubuntu-18.04',

    -- The name of the distribution.  This identifies the WSL distribution.
    -- It must match a valid distribution from your `wsl -l -v` output in
    -- order for the domain to be useful.
    distribution = 'Ubuntu-18.04',

    -- The username to use when spawning commands in the distribution.
    -- If omitted, the default user for that distribution will be used.

    -- username = "hunter",

    -- The current working directory to use when spawning commands, if
    -- the SpawnCommand doesn't otherwise specify the directory.

    -- default_cwd = "/tmp"

    -- The default command to run, if the SpawnCommand doesn't otherwise
    -- override it.  Note that you may prefer to use `chsh` to set the
    -- default shell for your user inside WSL to avoid needing to
    -- specify it here

    -- default_prog = {"fish"}
  },
}
```
