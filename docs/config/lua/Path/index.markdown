# `Path` object

{{since('nightly')}}

`Path` represents a string describing a path.

`Path` implements

- a `__tostring` meta method, which gives you back the string
describing the path (assuming the string is valid UTF-8). Thus, you can easily
use a `Path` object with `string.format` or `print` or `tostring`.
- a `__concat` meta method, which allows you to concatenate a `Path` with another
`Path` object or a string as if you are concatenating strings.
- an `__eq` meta method, which allows you to check if two `Path` objects are equal.

*Note:* Concatenation of the form `path1 .. path2` or `path .. string` will work
as expected, but `string .. path` will not work, since Lua uses the `string.__concat`
meta method in this case.

`Path` also implements the following string methods:

- `byte`
- `find`
- `gmatch`
- `gsub`
- `len`
- `lower`
- `match`
- `rep`
- `reverse`
- `sub`
- `upper`

*Note:* These are all the `string` functions in Lua that don't start with either
something other than a string or a format string. These methods are implemented
in such a way that they transform the given `Path` object to a string and then
calls the standard `string` method on this string. For technical reasons this
means that the methods will only work on strings that are valid UTF-8.

Additionally `Path` has the following methods:

