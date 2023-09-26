---
title: wezterm.read_dir
tags:
 - utility
 - filesystem
---
# `wezterm.read_dir(path [, callback])`

{{since('20200503-171512-b13ef15f')}}

This function returns an array containing the absolute file names of the
directory specified.  Due to limitations in the lua bindings, all of the paths
must be able to be represented as UTF-8 or this function will generate an
error.

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
where `filepath` is a Lua string of the entry's path and meta is a special `MetaData`
object for the entry.

If you want to use the function to change the output of read_dir, you should make sure
to return values in one of the two following forms:
```
bool,
{ bool, integer },
{ bool, integer, integer },
{ bool, integer, integer, integer },
and so on...
```

If the function returns `true`, then the given `filepath` will be included in the output
of `read_dir`, and if the function returns `false`, then the given `filepath` will be
excluded from the output.

If you return an array `{ bool, integer... }`, then the boolean will have the same meaning
as above, and the integers will be used for sorting (in the given order). (See below.)

If the function returns anything other than boolean or an array starting with a boolean,
or if we don't include the optional function at all, then the default behavior is to include
all `filepath`s.

The `MetaData` object (and thus`meta`) contains information about the entry (either
a directory, a file or a symlink), which can be accessed with the following methods:

* `is_dir` - returns true if the entry is a directory and false otherwise
* `is_file` - returns true if the entry is a file and false otherwise
* `is_symlink` - returns true if the entry is symlink and false otherwise
* `is_readonly` - returns true if the entry is readonly and false otherwise
* `secs_since_modified` - returns an integer with the number of seconds since
  the entry was last modified
* `secs_since_accessed` - returns an integer with the number of seconds since
  the entry was last accessed
* `secs_since_created` - returns an integer with the number of seconds since
  the entry was created
* `bytes` - returns the size of the entry in bytes

### Examples

If we want `read_only` to return the name (not path) of all folders and symlinks that
are not hidden files or folders (i.e., not starting with a `.`) in the home directory,
and then sort them first by the time since we last accessed the entry and thereafter
by the length of the filepath, we can do the following:

```lua
string.startswith = function(str, start)
  return str:sub(1, #start) == start
end

string.basename = function(s)
  return string.gsub(s, '(.*[/\\])(.*)', '%2')
end

local wezterm = require 'wezterm'
local home = wezterm.home_dir

for _, v in
  ipairs(wezterm.read_dir(home, function(filepath, meta)
    return {
      (meta:is_symlink() or meta:is_dir())
        and (not filepath:basename():startswith '.'),
      meta:secs_since_accessed(),
      #filepath,
    }
  end))
do
  wezterm.log_info('entry: ' .. v:basename())
end
```

Note: The purpose of sorting multiple times is that each sort don't swap equal values,
so if we for example have a lot of entries with the same length filepath, then we can
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

