---
title: wezterm.read_dir
tags:
 - utility
 - filesystem
---
# `wezterm.read_dir(path [, callback])`

{{since('20200503-171512-b13ef15f')}}

This function returns an array containing [`Path`](../Path/index.md) objects
{{since('nightly', inline=True)}} (strings before that) with the absolute file
names of the directory specified.
Due to limitations in the lua bindings, all of the paths must be able to be represented
as UTF-8 or this function will generate an error.

```lua
local wezterm = require 'wezterm'

-- logs the names of all of the entries under `/etc`
for _, v in ipairs(wezterm.read_dir '/etc') do
  wezterm.log_error('entry: ' .. v)
end
```

{{since('nightly')}}

`read_dir` accepts an optional callback function that can be used on each entry
of the directory being read with `read_dir`. The callback function should be of the form
```
function(filepath, meta)
  -- do things with the filepath and meta
end
```
where `filepath` is a (`Path`)[../Path/index.markdown] object and `meta` is a
(`MetaData`)[../MetaData/index.markdown] object for the entry.

*Note:* `meta` is the `MetaData` object you get from `filepath:metadat()`, so symbolic
links have been traversed to get it. If you want to know if the filepath is a symbolic
link you can use `filepath:symlink_metadata():is_symlink()`.

If you want to use the function to change the output of read_dir, you should make sure
to return values in one of the two following forms:
```
bool,
{ bool, path_or_string },
{ bool, integer... },
{ bool, path_or_string, integer... },
```

If the function returns `true`, then the given `filepath` will be included in the output
of `read_dir`, and if the function returns `false`, then the given `filepath` will be
excluded from the output.

If you return an array `{ bool, ... }`, then the boolean will have the same meaning
as above, while the optional string will be used for the name of the entry, and the optional
integers will be used for sorting (in the given order). (See below.)

If the function returns anything other than boolean or an array starting with a boolean,
or if we don't include the optional function at all, then the default behavior is to include
all `filepath`s.

### Examples

If we want `read_only` to return the name (not full path) of all folders that
are not hidden folders (i.e., not starting with a `.`) in the home directory,
and then sort them first by the time since we last accessed the entry and thereafter
by the length of the name, we can do the following:

```lua
local wezterm = require 'wezterm'
local home = wezterm.home_dir

tbl = wezterm.read_dir(home, function(filepath, meta)
  return {
    meta:is_dir() and (not filepath:basename():starts_with '.'),
    filepath:basename(),
    meta:secs_since_accessed(),
    #(filepath:basename()),
  }
end)
wezterm.log_info(tbl)
```

*Note:* The purpose of sorting multiple times is that each sort don't swap equal values,
so if we for example have a lot of entries with the same length names, then we can
make sure that entries of each length are additionally sorted by when they were last
accessed.

If we want a list of the path of all files of size at least 1kB and a most 10MB
that we created less than a year ago, and that is sorted from oldest to newest by creation
time, we can do the following:

```lua
local wezterm = require 'wezterm'
local home = wezterm.home_dir

local year_in_secs = 60 * 60 * 24 * 365

local tbl = wezterm.read_dir(home, function(filepath, meta)
  return {
    meta:is_file()
      and (10 ^ 3 < meta:bytes() and meta:bytes() < 10 ^ 7)
      and (meta:secs_since_created() < year_in_secs),
    -meta:secs_since_created(), -- we do minus to reverse the order
  }
end)
wezterm.log_info(tbl)
```

If we just want to list all directories in the home directory, we can do the following:
```lua
local wezterm = require 'wezterm'
local home = wezterm.home_dir

local tbl = wezterm.read_dir(home, function(filepath, meta)
  return meta:is_dir()
end)
wezterm.log_info(tbl)
```

