# `Path` object

{{since('nightly')}}

`Path` represents a string describing a path.

`Path` implements
* a `__tostring` meta method, which gives you back the string
describing the path (assuming the string is valid UTF-8). Thus, you can easily
use a `Path` object with `string.format` or `print` or `tostring`.
* a `__concat` meta method, which allows you to concatenate a `Path` with another
`Path` object or a string as if you are concatenating strings.
* an `__eq` meta method, which allows you to check if two `Path` objects are equal.

*Note:* Concatenation of the form `path1 .. path2` or `path .. string` will work
as expected, but `string .. path` will not work, since Lua uses the `string.__concat`
meta method in this case.

`Path` has the following methods:

