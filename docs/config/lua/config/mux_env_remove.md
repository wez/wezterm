---
tags:
  - multiplexing
  - spawn
---
# `mux_env_remove`

{{since('20211204-082213-a66c61ee9')}}

Specifies a list of environment variables that should be removed
from the environment in the multiplexer server.

The intent is to clean up environment variables that might give the wrong
impression of their operating environment to the various terminal sessions
spawned by the multiplexer server.

The default value for this is:

```lua
config.mux_env_remove = {
  'SSH_AUTH_SOCK',
  'SSH_CLIENT',
  'SSH_CONNECTION',
}
```
