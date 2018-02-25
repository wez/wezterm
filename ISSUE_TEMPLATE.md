Please include as much information as possible that can help to reproduce and understand the issue;
some pointers and suggestions are included here in this template.  You are empowered to include
more or less information than is asked for here!

## Is it a build problem?

Please include the output from these commands in this issue:

```
rustup show
cargo build --release
```

## Did something not work the way you expected?

If you can reproduce the issue please use the `wt-record` script to run `wezterm` and
record a transcript and include that in your issue.  This requires the `script` utility
to be installed on your system.

In the example below a file named `20180225161026.tgz` is produced.  Please attach that
file to this issue, or if it contains private or sensitive issue that you don't want the
public to see on GitHub, please find some other way to get that file to a project
contributor (perhaps Dropbox or email?).

```
$ ./wt-record
Transcript recorded in 20180225161026.tgz
```

You can use `wt-replay 20180225161026.tgz` to replay that file.

`wt-record` can only record the terminal output; it cannot record the input events going
in to the terminal, so if you are having an issue with input, please be sure to describe
it below!

### What did you try?

*Replace me with some context on what you were doing when you encountered the issue.*

### What did you expect to happen?

*Replace me with a description of your expectations.*

### What actually happened?

*Replace me with a description of what happened and how that differed from your expectations.*
