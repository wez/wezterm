# canonicalize_pasted_newlines

*Since: nightly builds only*

Controls whether pasted text will have newlines normalized to CRLF form.

In general wezterm tries to stick with unix line endings as the one-true
representation because using canonical CRLF can result in excess blank lines
during a paste operation.

On Windows we're in a bit of a frustrating situation: pasting into
Windows console programs requires CRLF otherwise there is no newline
at all, but when in WSL, pasting with CRLF gives excess blank lines.

By default, when `canonicalize_pasted_newlines` is not set in your
configuration, if wezterm is running as a native Windows application, then the
effective value of this setting will be `true`, otherwise it will be false.

The behavior of this setting is:

* If bracketed paste mode is enabled by the application, this configuration has no effect on the pasted text
* Otherwise, if `canonicalize_line_endings == true`, then the line endings will be converted to `CRLF` form

In practice, the default setting means that unix shells and vim will get the
unix newlines in their pastes (which is the UX most users will want) and
cmd.exe will get CRLF.

However, it is an imperfect world: some users take great pains to only run
unixy programs from their Windows wezterm, which means that they end up with
CRLFs in places where they don't want them.  Those users will likely wish to
set their configuration like this:

```lua
return {
  -- I only ever run unix programs, even on my Windows system, so I always
  -- want my pastes to use unix newlines.
  canonicalize_pasted_newlines = false,
}
```

