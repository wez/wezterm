---
tags:
  - exit_behavior
---
## `clean_exit_codes = { 0 }`

{{since('20220624-141144-bd1b7c5d')}}

Defines the set of exit codes that are considered to be a "clean" exit by
[exit_behavior](exit_behavior.md) when the program running in the terminal
completes.

Acceptable values are an array of integer exit codes that you wish to treat
as successful.

For example, if you often `CTRL-C` a program and then `CTRL-D`, bash will
typically exit with status `130` to indicate that a program was terminated
with SIGINT, but that bash itself wasn't.  In that situation you may wish
to set this config to treat `130` as OK:

```lua
config.clean_exit_codes = { 130 }
```

Note that `0` is always treated as a clean exit code and can be omitted
from the list.
