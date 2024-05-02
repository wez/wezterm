# `wezterm.table.flatten(array_of_arrays [, behavior])`

{{since('nightly')}}

This function flattens a list of Lua arrya-style tables, producing a single array-style
table comprised of the values of each of the tables in the input list.

For each table in the `array_of_arrays` parameter, the values are iterated over and put
into the return array.

The optional `behavior` parameter controls how deeply we flatten the array-like lists;
the accepted values are:

* `"Shallow"` (this is the default) - always take the latest value for a key, even if
  the same key has already been populated into the return value, forcing the
  existing value to be updated with a later value.

* `"Deep"` - keep the first value of the key. Subsequent values for that same key
  are ignored.

*Note:* `flatten` will ignore non-array like keys when going through the tables. I.e.,
it goes through contiguous integer keys starting with index `1`. This is similar to the
behavior of `ipairs`, and not `pairs`. For behavior like `pairs`, see
[wezterm.table.extend](extend.md).

```lua
local wezterm = require 'wezterm'
local flatten = wezterm.table.flatten
local equal = wezterm.table.equal

local arr1 = { { 1, 2 }, 3 }
local arr2 = { 'a', { 'b', { 'c' } } }
local arr3 = { 1, { a = 1, 2 }, { b = 2 } }

assert(equal(flatten { arr1, arr2 }, { { 1, 2 }, 3, 'a', { 'b', { 'c' } } }))
assert(equal(flatten({ arr1, arr2 }, 'Deep'), { 1, 2, 3, 'a', 'b', 'c' }))

assert(
  equal(flatten { arr1, arr3 }, { { 1, 2 }, 3, 1, { a = 1, 2 }, { b = 2 } })
)
assert(equal(flatten({ arr1, arr3 }, 'Deep'), { 1, 2, 3, 1, 2 }))
```

See also [wezterm.table.extend](extend.md).
