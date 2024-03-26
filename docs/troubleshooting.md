# Troubleshooting

## Review logs/error messages

If things aren't working out, there may be an issue printed in the logs.
Read on to learn more about how to see those logs.

### Debug Overlay

By default, pressing <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd> will activate
the debug overlay and allow you to review the most recently logged issues.
It also gives you access to a Lua REPL for evaluating built-in lua functions.

See [ShowDebugOverlay](config/lua/keyassignment/ShowDebugOverlay.md) for more
information on this key assignment.

### Log Files

You can find log files in `$XDG_RUNTIME_DIR/wezterm` on unix systems,
or `$HOME/.local/share/wezterm` on macOS and Windows systems.

### Increasing Log Verbosity

The `WEZTERM_LOG` environment variable can be used to adjust the level
of logging for different modules within wezterm.

To see maximum verbosity, you can start wezterm like this:

```
WEZTERM_LOG=debug wezterm
```

to see debug level logs for everything on stdout.

On Windows systems you'll usually need to set the environment variable separately:

Using `cmd.exe`:

```
C:\> set WEZTERM_LOG=debug
C:\> wezterm
```

Using powershell:

```
PS C:\> $env:WEZTERM_LOG="debug"
PS C:\> wezterm
```

When using a flatpak you must first enter the flatpak container by running:

```
flatpak run --command=sh --devel org.wezfurlong.wezterm
```

Before then running `wezterm`.

Each log line will include the module name, which is a colon separated
namespace; in the output below the modules are `config`,
`wezterm_gui::frontend`, `wezterm_font::ftwrap` and `wezterm_gui::termwindow`:

```
10:29:24.451  DEBUG  config                    > Reloaded configuration! generation=2
10:29:24.452  DEBUG  wezterm_gui::frontend     > workspace is default, fixup windows
10:29:24.459  DEBUG  wezterm_font::ftwrap      > set_char_size computing 12 dpi=124 (pixel height=20.666666666666668)
10:29:24.461  DEBUG  wezterm_font::ftwrap      > set_char_size computing 12 dpi=124 (pixel height=20.666666666666668)
10:29:24.494  DEBUG  wezterm_gui::termwindow   > FocusChanged(true)
10:29:24.495  DEBUG  wezterm_gui::termwindow   > FocusChanged(false)
```

Those modules generally match up to directories and file names within the
wezterm source code, or to external modules that wezterm depends upon.

You can set a more restrictive filter to focus in on just the things you want.
For example, if you wanted to debug only configuration related things you might
set:

```
WEZTERM_LOG=config=debug,info
```

which says:

* log `config` at `debug` level
* everything else at `info` level

You can add more comma-separated items:

```
WEZTERM_LOG=config=debug,wezterm_font=debug,info
```

See Rust's [env_logger
documentation](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)
for more details on the syntax/possibilities.

## Debugging Keyboard Related issues

Turn on [debug_key_events](config/lua/config/debug_key_events.md) to log
information about key presses.

Use [wezterm show-keys](cli/show-keys.md) or `wezterm show-keys --lua` to show
the effective set of key and mouse assignments defined by your config.

Consider changing [use_ime](config/lua/config/use_ime.md) to see that is
influencing your keyboard usage.

Double check to see if you have some system level utility/software that might
be intercepting or changing the behavior of a keyboard shortcut that you're
trying to use.

## Debugging Font Display

Use `wezterm ls-fonts` to explain which fonts will be used for different styles
of text.

Use `wezterm ls-fonts --list-system` to get a list of fonts available on your
system, in a form that you can use in your config file.

Use `wezterm ls-fonts --text foo` to explain how wezterm will render the text
`foo`, and `wezterm ls-fonts --text foo --rasterize-ascii` to show an ascii art
rendition of that text.

