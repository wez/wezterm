
# WezTerm Recipes

## Inbuilt TMUX/GNU Screen style multiplexed sessions without using a terminal multiplexor

WezTerm has inbuilt terminal multiplexing capability without needing to use a
separate dedicated terminal multiplexor like tmux or GNU Screen. You can
configure a unix domain with automatic connect to a detached domain and achieve
tmux/screen functionality.

```lua
return {
    unix_domains = {
    {
        name = "unix",
        connect_automatically = true
    }
}
```

With this you can reconnect to the detached session by launching the wezterm
app or by running `wezterm connect unix` from a command line.
