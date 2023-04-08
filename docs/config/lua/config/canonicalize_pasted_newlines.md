---
tags:
  - clipboard
---
# `canonicalize_pasted_newlines`

{{since('20211204-082213-a66c61ee9')}}

Controls whether pasted text will have newlines normalized.

If bracketed paste mode is enabled by the application, the effective
value of this configuration option is `"None"`.

The following values are accepted:

|value|meaning|version|
|-----|-------|---------------|
|`true` |same as `"CarriageReturnAndLineFeed"`|{{since('20211204-082213-a66c61ee9', inline=True)}}|
|`false` |same as `"None"`|{{since('20211204-082213-a66c61ee9', inline=True)}}|
|`"None"` |The text is passed through unchanged|{{since('20220319-142410-0fcdea07', inline=True)}}|
|`"LineFeed"` |Newlines of any style are rewritten as LF|{{since('20220319-142410-0fcdea07', inline=True)}}|
|`"CarriageReturn"` |Newlines of any style are rewritten as CR|{{since('20220319-142410-0fcdea07', inline=True)}}|
|`"CarriageReturnAndLineFeed"` |Newlines of any style are rewritten as CRLF|{{since('20220319-142410-0fcdea07', inline=True)}}|

Note that the string forms of these values are accepted in 20220319-142410-0fcdea07,
however, `true` in all prior versions behaves the same way as
`"CarriageReturnAndLineFeed"` behaves in the nightly build.

The default value has changed in different versions of wezterm:

|version|platform|default|
|-------|--------|-------|
|20211204-082213-a66c61ee9|Windows|`"CarriageReturnAndLineFeed"`|
|20211204-082213-a66c61ee9|NOT Windows|`"None"`|
|20220319-142410-0fcdea07|NOT Windows|`"CarriageReturn"`|

On Windows we're in a bit of a frustrating situation: pasting into
Windows console programs requires CRLF otherwise there is no newline
at all, but when in WSL, pasting with CRLF gives excess blank lines.

In practice, the default setting means that unix shells and vim will get the
unix newlines in their pastes (which is the UX most users will want) and
cmd.exe will get CRLF.

