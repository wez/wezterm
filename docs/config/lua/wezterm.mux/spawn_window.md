## `wezterm.mux.spawn_window{}`

*Since: 20220624-141144-bd1b7c5d*

Spawns a program into a new window, returning the [MuxTab](../MuxTab/index.md),
[Pane](../pane/index.md) and [MuxWindow](../mux-window/index.md) objects
associated with it:

```lua
local tab, pane, window = wezterm.mux.spawn_window {}
```

When no arguments are passed, the default program is spawned.

The following parameters are supported:

### args

Specifies the argument array for the command that should be spawned.
If omitted the default program for the domain will be spawned.

```lua
wezterm.mux.spawn_window { args = { 'top' } }
```

### cwd

Specify the current working directory that should be used for
the program.

If unspecified, follows the rules from [default_cwd](../config/default_cwd.md)

```lua
wezterm.mux.spawn_window { cwd = '/tmp' }
```

### set_environment_variables

Sets additional environment variables in the environment for
this command invocation.

```lua
wezterm.mux.spawn_window { set_environment_variables = { FOO = 'BAR' } }
```

### domain

Specifies the multiplexer domain into which the program should
be spawned.  The default value is assumed to be `"DefaultDomain"`,
which causes the default domain to be used.

You may specify the name of one of the multiplexer domains
defined in your configuration using the following:

```lua
wezterm.mux.spawn_window { domain = { DomainName = 'my.name' } }
```

### width and height

Only valid when width and height are used together, allows specifying
the number of column and row cells that the window should have.

```lua
wezterm.mux.spawn_window { width = 60, height = 30 }
```

### workspace

Specifies the name of the workspace that the newly created window
will be associated with.  If omitted, the currently active workspace
name will be used.

```lua
wezterm.mux.spawn_window { workspace = { 'coding' } }
```


