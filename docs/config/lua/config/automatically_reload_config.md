---
tags:
  - reload
---
# `automatically_reload_config`

{{since('20201031-154415-9614e117')}}

When true (the default), watch the config file and reload it
automatically when it is detected as changing.
When false, you will need to manually trigger a config reload
with a key bound to the action [ReloadConfiguration](../keyassignment/ReloadConfiguration.md).

For example, to disable auto config reload:

```lua
config.automatically_reload_config = false
```
