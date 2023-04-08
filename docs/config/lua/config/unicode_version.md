---
tags:
  - unicode
---
# `unicode_version = 9`

{{since('20211204-082213-a66c61ee9')}}

Specifies the version of unicode that will be used when interpreting the
width/presentation of text.

This option exists because Unicode is an evolving specification that introduces
new features and that occasionally adjusts how existing features should be
handled.

For example, there were a number of unicode code points that had their width
changed between Unicode version 8 and version 9. This wouldn't be an issue
if all software was simultaneously aware of the change, but the reality is
that there is a lot of older software out there, and that even if your local
system is fully up to date, you might connect to a remote system vis SSH
that is running applications that use a different version of unicode than
your local system.

The impact of mismatching expectations of unicode width for a terminal emulator
is that text columns may no longer line up as the application author expected,
and/or that the cursor may appear to be in the wrong place when editing lines
or text in shells or text editors.

The `unicode_version` option defaults to unicode version 9 as that is the most
widely used version (from the perspective of width) at the time of writing,
which means that the default experience has the lowest chance of mismatched
expectations.

| Unicode Version | Impact |
| --------------- | ------ |
| 8 (or lower)    | Some characters will be narrower than later versions |
| 9-13            | Some characters will be wider than in Unicode 8 |
| 14 (or higher)  | Explicit Emoji or Text presentation selectors will be respected and make some characters wider or narrower than earlier versions, depending on the context |

If you aggressively maintain all of your software to the latest possible
versions then you may wish to set `unicode_version = 14` to match the current
(at the time of writing) version of Unicode.  This will enable Emoji
Presentation selectors to affect the presentation of certain emoji characters
and alter their width in the terminal display.

If you'd like to use a higher default version but switch to a lower version
when launching an older application, or when SSH'ing into a remote host, then
you may be pleased to learn that wezterm also provides an escape sequence that
allows the unicode version to be changed on the fly.

## Unicode Version Escape sequence

This escape sequence is was originally defined by iTerm2. It supports setting
the value as well as pushing and popping the value on a stack, which is helpful
when temporarily adjusting the value.

```
OSC 1337 ; UnicodeVersion=N ST
```

The above sets the unicode version to `N`, where N is the integer version number.

```
OSC 1337 ; UnicodeVersion=push ST
```

Pushes the current version onto a stack.

```
OSC 1337 ; UnicodeVersion=pop ST
```

Pops the last-pushed version from the stack and sets the unicode version to that value.
If there were no entries on the stack, the unicode version is left unchanged.

```
OSC 1337 ; UnicodeVersion=push LABEL ST
```

Pushes the current version onto a stack, labeling it with `LABEL`.

```
OSC 1337 ; UnicodeVersion=pop LABEL ST
```

Pops entries from the stack stopping after an entry labelled with `LABEL` is popped.


The labels are helpful when writing a wrapper alias, for example:

```bash
function run_with_unicode_version_9() {
  local label=$(uuidgen)
  printf "\e]1337;UnicodeVersion=push %s\e\\" $label
  printf "\e]1337;UnicodeVersion=9\e\\"
  eval $@
  local result=${PIPESTATUS[0]}
  printf "\e]1337;UnicodeVersion=pop %s\e\\" $label
  return $result
}

# Connect to a remote machine with an older version of unicode
run_with_unicode_version_9 ssh remote.machine
```

Will save the current version on the stack with a unique label, then set the version to `9`
and spawn the requested command.  When the command returns it will restore the saved
version, even if the command itself set or pushed other values.

