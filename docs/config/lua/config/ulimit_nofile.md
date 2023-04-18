---
tags:
  - tuning
---
# `ulimit_nofile = 2048`

{{since('20230408-112425-69ae8472')}}

On Unix systems, specifies the minimum desirable value for the `RLIMIT_NOFILE`
*soft limit*.

That system parameter controls the maximum number of file descriptors that a
given process is permitted to open.

On startup, wezterm will inspect the soft and hard limits, and if the soft
limit is *below* the value of the `ulimit_nofile` option, wezterm will attempt to
raise it to `min(ulimit_nofile, hard_limit)`.

