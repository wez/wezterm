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

`Path` has the following methods:



  - [ancestors](ancestors.md)
  - [basename](basename.md)
  - [canonical_path](canonical_path.md)
  - [clone](clone.md)
  - [components](components.md)
  - [dirname](dirname.md)
  - [ends_with](ends_with.md)
  - [extension](extension.md)
  - [file_stem](file_stem.md)
  - [is_absolute](is_absolute.md)
  - [is_relative](is_relative.md)
  - [join](join.md)
  - [metadata](metadata.md)
  - [pop](pop.md)
  - [push](push.md)
  - [read_dir](read_dir.md)
  - [read_link](read_link.md)
  - [set_extension](set_extension.md)
  - [set_filename](set_filename.md)
  - [starts_with](starts_with.md)
  - [strip_prefix](strip_prefix.md)
  - [symlink_metadata](symlink_metadata.md)
  - [try_exists](try_exists.md)
