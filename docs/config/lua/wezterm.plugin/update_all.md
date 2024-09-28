# update_all function

{{since('20230320-124340-559cb7b0')}}

Attempt to fast-forward or `pull --rebase` each of the repos in the plugin directory.

!!! Note

    The configuration is **not** reloaded afterwards; the user will need to do that themselves.

!!! Tip

    Run the [`wezterm.reload_configuration()`](../wezterm/reload_configuration.md) function to reload the configuration.

