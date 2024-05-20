---
tags:
  - multiplexing
  - ssh
---
# `default_ssh_auth_sock`

{{since('nightly')}}

Setting this value will cause wezterm to replace the the value of the
`SSH_AUTH_SOCK` environment when it first starts up, and to use this value for
the auth socket registered with the multiplexer server (visible via `wezterm
cli list-clients`).

You won't normally need to set this, but if you are running with an alternative
identity agent and want to replace the default on your system, this gives
you that ability.

For example, @wez currently uses the 1Password SSH Auth Agent, but when
running on Gnome the system default is Gnome's keyring agent.

While you can fix this up in your shell startup files, those are not involved
when spawning the GUI directly from the desktop environment.

The following wezterm configuration snippet shows how to detect when gnome
keyring is set and to selectively replace it with the 1Password agent:

```lua
local config = wezterm.config_builder()

-- Override gnome keyring with 1password's ssh agent
local SSH_AUTH_SOCK = os.getenv 'SSH_AUTH_SOCK'
if
  SSH_AUTH_SOCK
  == string.format('%s/keyring/ssh', os.getenv 'XDG_RUNTIME_DIR')
then
  local onep_auth =
    string.format('%s/.1password/agent.sock', wezterm.home_dir)
  -- Glob is being used here as an indirect way to check to see if
  -- the socket exists or not. If it didn't, the length of the result
  -- would be 0
  if #wezterm.glob(onep_auth) == 1 then
    config.default_ssh_auth_sock = onep_auth
  end
end
```

