---
tags:
  - multiplexing
---
# `default_domain = "local"`

{{since('20220319-142410-0fcdea07')}}

!!! note
    This option only applies to the GUI.  For the equivalent option in
    the standalone mux server, see [default_mux_server_domain](default_mux_server_domain.md)

When starting the GUI (not using the `serial` or `connect` subcommands), by default wezterm will set the built-in `"local"` domain as the default multiplexing domain.

The `"local"` domain represents processes that are spawned directly on the local system.

Windows users, particularly those who use
[WSL](https://docs.microsoft.com/en-us/windows/wsl/about), may wish to override
the default domain to instead use a particular WSL distribution so that wezterm
launches directly into a Linux shell rather than having to manually invoke
`wsl.exe`.  Using a [WslDomain](../WslDomain.md) for this has the advantage
that wezterm can then use [shell integration](../../../shell-integration.md) to
track the current directory inside WSL and use it when splitting new panes or
spawning new tabs.

For example, if:

```
; wsl -l -v
  NAME            STATE           VERSION
* Ubuntu-18.04    Running         1
```

then wezterm will by default create a `WslDomain` with the name `"WSL:Ubuntu-18.04"`
and if I set my config like this:

```lua
config.default_domain = 'WSL:Ubuntu-18.04'
```

then when wezterm starts up, it will open with a shell running inside that Ubuntu
distribution rather than using the default `cmd` or `powershell`.

While these examples are WSL-centric, `default_domain` will accept the name
of any of the available [multiplexing domains](../../../multiplexing.md).
