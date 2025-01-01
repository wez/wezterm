---
tags:
  - tuning
---
# `ulimit_nproc = 2048`

{{since('20230408-112425-69ae8472')}}

On Unix systems, specifies the minimum desirable value for the `RLIMIT_NPROC`
*soft limit*.

That system parameter controls the maximum number of simultaneous processes
that a given user is permitted to spawn.

On startup, wezterm will inspect the soft and hard limits, and if the soft
limit is *below* the value of the `ulimit_nproc` option, wezterm will attempt to
raise it to `min(ulimit_nproc, hard_limit)`.


