The purpose of this crate is to make it a bit more ergonomic for portable
applications that need to work with the platform level `RawFd` and
`RawHandle` types.

Rather than conditionally using `RawFd` and `RawHandle`, the `FileDescriptor`
type can be used to manage ownership, duplicate, read and write.

## FileDescriptor

This is a bit of a contrived example, but demonstrates how to avoid
the conditional code that would otherwise be required to deal with
calling `as_raw_fd` and `as_raw_handle`:

```rust
use filedescriptor::{FileDescriptor, FromRawFileDescriptor};
use failure::Fallible;
use std::io::Write;

fn get_stdout() -> Fallible<FileDescriptor> {
  let stdout = std::io::stdout();
  let handle = stdout.lock();
  FileDescriptor::dup(&handle)
}

fn print_something() -> Fallible<()> {
   get_stdout()?.write(b"hello")?;
   Ok(())
}
```

## Pipe
The `Pipe` type makes it more convenient to create a pipe and manage
the lifetime of both the read and write ends of that pipe.

```rust
use filedescriptor::Pipe;
use std::io::{Read,Write};
use failure::Error;

let mut pipe = Pipe::new()?;
pipe.write.write(b"hello")?;
drop(pipe.write);

let mut s = String::new();
pipe.read.read_to_string(&mut s)?;
assert_eq!(s, "hello");
# Ok::<(), Error>(())
```

