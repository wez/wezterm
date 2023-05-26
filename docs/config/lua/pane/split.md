# `pane:split{}`

{{since('20220624-141144-bd1b7c5d')}}

Splits `pane` and spawns a program into the split, returning the
`Pane` object associated with it:

```lua
local new_pane = pane:split {}
```

When no arguments are passed, the pane is split in half left/right and the
right half has the default program spawned into it.

The following parameters are supported:

### args

Specifies the argument array for the command that should be spawned.
If omitted the default program for the domain will be spawned.

```lua
pane:split { args = { 'top' } }
```

### cwd

Specify the current working directory that should be used for
the program.

If unspecified, follows the rules from [default_cwd](../config/default_cwd.md)

```lua
pane:split { cwd = '/tmp' }
```

### set_environment_variables

Sets additional environment variables in the environment for
this command invocation.

```lua
pane:split { set_environment_variables = { FOO = 'BAR' } }
```

### domain

Specifies the multiplexer domain into which the program should
be spawned.  The default value is assumed to be `"CurrentPaneDomain"`,
which causes the default domain to be used.

You may specify the name of one of the multiplexer domains
defined in your configuration using the following:

```lua
pane:split { domain = { DomainName = 'my.name' } }
```

Or you may use the default domain:

```lua
pane:split { domain = 'DefaultDomain' }
```

### direction

Specifies where the new pane should be placed.  Possible values are:

* `"Right"` - splits the pane left/right and places the new pane on the right.
* `"Left"` - splits the pane left/right and places the new pane on the left.
* `"Top"` - splits the pane top/bottom and places the new pane on the top.
* `"Bottom"` - splits the pane top/bottom and places the new pane on the bottom.

```lua
pane:split { direction = 'Top' }
```

### top_level

If `true`, rather than splitting `pane` in half, the tab that contains it
is split and the new pane runs the full extent of the tab dimensions.

```lua
pane:split { direction = 'Bottom', top_level = true }
```

### size

Controls the size of the new pane.

Numeric values less than `1.0` are used to express a fraction of the
available space; `0.5` means `50%` of the available space, for example.

Numeric values greater or equal to `1` are used to specify the number of
cells.

The default value is `0.5`.

This creates two additional splits within `pane`, creating three
total splits that each occupy 1/3 of the available space:

```lua
pane:split { direction = 'Top', size = 0.333 }
pane:split { direction = 'Top', size = 0.5 }
```

