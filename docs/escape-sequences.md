> Please note that this is a "living document" and may lag or lead the state
> of the current stable release in a number of areas--as you might imagine,
> precisely documenting escape codes and their behaviors and cross-checking
> with the various technical documents is laborious and tedious and I only
> have so much spare time!
>
> If you notice that something is inaccurate or missing, please do [file an issue](https://github.com/wezterm/wezterm/issues/new/choose)
> so that it can be resolved!

## Output/Escape Sequences

WezTerm considers the output from the terminal to be a UTF-8 encoded stream of
codepoints.  No other encoding is supported.  As described below, some C1
control codes have both 7-bit ASCII compatible as well as 8-bit
representations.  As ASCII is a compatible subset of UTF-8, the 7-bit
representations are preferred and processed without any special consideration.

The 8-bit values *are* recognized, but only if the 8-bit value is treated as a
unicode code point and encoded via a UTF-8 multi-byte sequence.

### Printable Codepoints

Codepoints with value 0x20 and higher are considered to be printable and are
applied to the terminal display using the following rules:

* Codepoints are buffered until a C0, C1 or other escape/control sequence is encountered,
  which triggers a flush and processing continues with the next step.
* The buffered codepoint sequence is split into unicode graphemes, which means that
  combining sequences and emoji are decoded.  Processing continues for below for
  each individually recognized grapheme.
* If DEC line drawing mode is active, graphemes `j-n`, `q`, `t-x` are translated
  to equivalent line drawing graphemes and processing continues.
* If prior output/actions require it, the cursor position may be moved to a new line
  and the terminal display may be scrolled to make accommodate it.
* An appropriate number of cells, starting at the current cursor position,
  are allocated based on the column width of the current grapheme and are assigned
  to the grapheme.  The current current graphics rendition state (such as colors
  and other presentation attributes) is also applied to those cells.
  If insert mode is active, those cells will be inserted at the current cursor
  position, otherwise they will overwrite cells at the current cursor position.
* The cursor position will be updated based on the column width of the grapheme.

After the graphemes are applied to the terminal display, the rendering portion of
WezTerm will attempt to apply your [font shaping](config/font-shaping.md) configuration
based on runs of graphemes with matching graphic attributes to determine which glyphs
should be rendered from your fonts; it is at this stage that emoji and ligatures are
resolved.

### C0 Control Codes

Codepoints in the range `0x00-0x1F` are considered to be `C0` control codes.
`C0` controls will flush any buffered printable codepoints before triggering
the action described below.

| Seq|Hex |Name|Description|Action |
|----|----|----|-----------|------ |
| ^@ |0x00|NUL |Null       |Ignored|
| ^A |0x01|SOH |Start of Heading|Ignored|
| ^B |0x02|STX |Start of Text|Ignored|
| ^C |0x03|ETX |End of Text|Ignored|
| ^D |0x04|EOT |End of Transmission|Ignored|
| ^E |0x05|ENQ |Enquiry    |Ignored|
| ^F |0x06|ACK |Acknowledge|Ignored|
| ^G |0x07|BEL |Bell       |Logs `Ding! (this is the bell)` to stderr of the WezTerm process. See [#3](https://github.com/wezterm/wezterm/issues/3)|
| ^H |0x08|BS  |Backspace  |Move cursor left by 1, constrained by the left margin. If Reverse Wraparound and dec auto wrap modes are enabled, moving left of the left margin will jump the cursor to the right margin, jumping to bottom right margin if it was at the top left.|
| ^I |0x09|HT  |Horizontal Tab|Move cursor right to the next tab stop|
| ^J |0x0A|LF  |Line Feed  |If cursor is at the bottom margin, scroll the region up, otherwise move cursor down 1 row|
| ^K |0x0B|VT  |Vertical Tab|Treated as Line Feed|
| ^L |0x0C|FF  |Form Feed   |Treated as Line Feed|
| ^M |0x0D|CR  |Carriage Return|If cursor is left of leftmost margin, move to column 0. Otherwise move to left margin|
| ^N |0x0E|SO  |Shift Out   |Ignored|
| ^O |0x0F|SI  |Shift In    |Ignored|
| ^P |0x10|DLE |Data Link Escape|Ignored|
| ^Q |0x11|DC1 |Device Control One|Ignored|
| ^R |0x12|DC2 |Device Control Two|Ignored|
| ^S |0x13|DC3 |Device Control Three|Ignored|
| ^T |0x14|DC4 |Device Control Four|Ignored|
| ^U |0x15|NAK |Negative Acknowledge|Ignored|
| ^V |0x16|SYN |Synchronous Idle|Ignored|
| ^W |0x17|ETB |End Transmission Block|Ignored|
| ^X |0x18|CAN |Cancel       |Ignored|
| ^Y |0x19|EM  |End of Medium|Ignored|
| ^Z |0x1A|SUB |Substitute   |Ignored|
| ^\[ |0x1B|ESC |Escape       |Introduces various escape sequences described below|
| ^\\|0x1C|FS  |File Separator|Ignored|
| ^] |0x1D|GS  |Group Separator|Ignored|
| ^^ |0x1E|RS  |Record Separator|Ignored|
| ^_ |0x1F|US  |Unit Separator|Ignored|

### C1 Control Codes

As mentioned above, WezTerm only supports UTF-8 encoding.  C1 control codes
have an 8-bit representation as well as a multi-codepoint 7-bit escape sequence.

The 8-bit representation is recognized if the 8-bit value is treated as a
unicode code point and encoded as a multi-byte UTF-8 sequence.  Sending the
8-bit binary value will not be recognized as intended, as those bitsequences
are passing through a UTF-8 decoder.

The table below lists the 7-bit `C1` sequence (which is preferred) as well as the
codepoint value, along with the corresponding meaning.

As with `C0` control codes, `C1` controls will flush any buffered printable
codepoints before triggering the action described below.

|Seq   |Codepoint|Name|Description       |Action|
|----- |---------|----|------------------|------|
|ESC D |0x84     |IND |Index             |Moves the cursor down one line in the same column. If the cursor is at the bottom margin, the page scrolls up|
|ESC E |0x85     |NEL |Next Line         |Moves the cursor to the left margin on the next line. If the cursor is at the bottom margin, scroll the page up|
|ESC H |0x88     |HTS |Horizontal Tab Set|Sets a horizontal tab stop at the column where the cursor is|
|ESC M |0x8D     |RI  |Reverse Index     |Move the cursor up one line. If the cursor is at the top margin, scroll the region down|
|ESC P |0x90     |DCS |Device Control String|Discussed below|
|ESC [ |0x9B     |CSI |Control Sequence Introducer|Discussed below|
|ESC \\|0x9C     |ST  |String Terminator |No direct effect; ST is used to delimit the end of OSC style escape sequences|

### Other Escape Sequences

As these sequences start with an `ESC`, which is a `C0` control, these will
flush any buffered printable codepoints before triggering the associated
action.

|Seq    | Name   | Description         | Action |
|-------|--------|---------------------|--------|
|ESC c  | [RIS](https://vt100.net/docs/vt510-rm/RIS.html) | Reset to Initial State | Resets tab stops, margins, modes, graphic rendition, palette, activates primary screen, erases the display and moves cursor to home position |
|ESC 7  | [DECSC](https://vt100.net/docs/vt510-rm/DECSC.html)  | Save Cursor Position| Records cursor position |
|ESC 8  | [DECRC](https://vt100.net/docs/vt510-rm/DECRC.html)  | Restored Saved Cursor Position | Moves cursor to location it had when DECSC was used |
|ESC =  | [DECPAM](https://vt100.net/docs/vt510-rm/DECPAM.html) | Application Keypad  | Enable Application Keypad Mode |
|ESC >  | [DECPNM](https://vt100.net/docs/vt510-rm/DECPNM.html) | Normal Keypad       | Set Normal Keypad Mode |
|ESC (0 |        | DEC Line Drawing character set (G0) | Translate characters `j-x` to line drawing glyphs |
|ESC (B |        | US ASCII character set (G0) | Disables DEC Line Drawing character translation |
|ESC (A |        | UK character set (G0) | Select the UK national character set for G0 |
|ESC )0 |        | DEC Line Drawing character set (G1) | Designate DEC line drawing to the G1 set |
|ESC )B |        | US ASCII character set (G1) | Designate US ASCII to the G1 set |
|ESC )A |        | UK character set (G1) | Designate the UK national character set to the G1 set |
|ESC #3 | [DECDHL](https://vt100.net/docs/vt510-rm/DECDHL.html) | Double-Height Line, top half | Marks the current line as the top half of a double-height line |
|ESC #4 | [DECDHL](https://vt100.net/docs/vt510-rm/DECDHL.html) | Double-Height Line, bottom half | Marks the current line as the bottom half of a double-height line |
|ESC #5 | [DECSWL](https://vt100.net/docs/vt510-rm/DECSWL.html) | Single-Width Line | Marks the current line as single-width (the default) |
|ESC #6 | [DECDWL](https://vt100.net/docs/vt510-rm/DECDWL.html) | Double-Width Line | Marks the current line as double-width |
|ESC #8 | [DECALN](https://vt100.net/docs/vt510-rm/DECALN.html) | Screen Alignment Display | Fills the display with `E` characters for diagnostic/test purposes (for vttest) |
|ESC k  |        | Set window title (tmux/screen) | Accumulates a title string until `ST`, then applies it as the window title |

A few other `ESC` codes are recognized by the parser but have no effect:
`ESC F` (cursor to lower left), `ESC 6` (DECBI, Back Index), `ESC N`/`ESC O`
(SS2/SS3 single shifts), `ESC Z` (DECID), `ESC V`/`ESC W` (SPA/EPA), and
`ESC X`/`ESC ^`/`ESC _` (SOS/PM/APC).

### CSI - Control Sequence Introducer Sequences

CSI sequences begin with the `C1` `CSI` sequence, which is either the 7-bit
`ESC [` sequence or the codepoint `0x9B`.

WezTerm classifies these sequences into a number of functional families which
are broken out below.

#### Graphic Rendition (SGR)

SGR sequences are of the form `CSI DIGITS [; DIGITS ]+ m`.  That is, any number
of semicolon separated numbers, terminated by the `m` codepoint.  There are a handful
of slightly more modern sequences that use colon `:` codepoints to encode additional
context.

The digits map to one of the codes in the table below, which manipulate the
presentation attributes of subsequently printed characters.

It is valid to omit the code number; for example `CSI m` is equivalent to `CSI
0 m` which resets the presentation attributes.

|Code|Description|Action|
|--- |-----------|------|
|0   |Reset      |Reset to default foreground/background colors, reset all presentation attributes, clear any explicit hyperlinks| 
|1   |IntensityBold|Set the intensity level to Bold.  This causes subsequent text to be rendered in a bold font variant and, if the foreground color is set to a palette index in the 0-7 range, effectively shifts it to the brighter value in the 8-15 range|
|2   |IntensityDim|Set the intensity level to Dim or Half-Bright.  This causes text to be rendered in a lighter weight font variant|
|3   |ItalicOn|Sets the italic attribute on the text, causing an italic font variant to be selected|
|4   |UnderlineOn|Text will have a single underline|
|4:0 |UnderlineOff|Text will have no underline|
|4:1 |UnderlineOn|Text will have a single underline|
|4:2 |UnderlineDouble|Text will be rendered with double underline|
|4:3 |UnderlineCurly|Text will be rendered with a curly underline|
|4:4 |UnderlineDotted|Text will be rendered with a dotted underline|
|4:5 |UnderlineDashed|Text will be rendered with a dashed underline|
|5   |BlinkOn|Indicates that the text should blink <150 times per minute|
|6   |RapidBlinkOn|Indicates that the text should blink >150 times per minute|
|7   |InverseOn|Causes the foreground and background colors to be swapped|
|8   |InvisibleOn|Marks text as invisible.|
|9   |StrikeThroughOn|Text will be rendered with a single line struck through the middle|
|21  |UnderlineDouble|Text will be rendered with double underline|
|22  |NormalIntensity|Cancels the effect of IntensityBold and IntensityDim, returning the text to normal intensity|
|23  |ItalicOff|Cancels the effect of ItalicOn|
|24  |UnderlineOff|Text will have no underline|
|25  |BlinkOff|Cancels the effect of BlinkOn and RapidBlinkOn|
|27  |InverseOff|Cancels the effect of InverseOn|
|28  |InvisibleOff|cancels the effect of InvisibleOn|
|29  |StrikeThroughOff|Cancels the effect of StrikeThroughOn|
|30  |ForegroundBlack|Sets the foreground color to ANSI Black, which is palette index 0|
|31  |ForegroundRed|Sets the foreground color to ANSI Red, which is palette index 1|
|32  |ForegroundGreen|Sets the foreground color to ANSI Green, which is palette index 2|
|33  |ForegroundYellow|Sets the foreground color to ANSI Yellow, which is palette index 3|
|34  |ForegroundBlue|Sets the foreground color to ANSI Blue, which is palette index 4|
|35  |ForegroundMagenta|Sets the foreground color to ANSI Magenta, which is palette index 5|
|36  |ForegroundCyan|Sets the foreground color to ANSI Cyan, which is palette index 6|
|37  |ForegroundWhite|Sets the foreground color to ANSI White, which is palette index 7|
|39  |ForegroundDefault|Sets the foreground color to the user's configured default text color|
|40  |BackgroundBlack|Sets the background color to ANSI Black, which is palette index 0|
|41  |BackgroundRed|Sets the background color to ANSI Red, which is palette index 1|
|42  |BackgroundGreen|Sets the background color to ANSI Green, which is palette index 2|
|43  |BackgroundYellow|Sets the background color to ANSI Yellow, which is palette index 3|
|44  |BackgroundBlue|Sets the background color to ANSI Blue, which is palette index 4|
|45  |BackgroundMagenta|Sets the background color to ANSI Magenta, which is palette index 5|
|46  |BackgroundCyan|Sets the background color to ANSI Cyan, which is palette index 6|
|47  |BackgroundWhite|Sets the background color to ANSI White, which is palette index 7|
|49  |BackgroundDefault|Sets the background color to the user's configured default background color|
|53  |OverlineOn|Renders text with a single overline/overbar|
|55  |OverlineOff|Cancels OverlineOn|
|59  |UnderlineColorDefault|Resets the underline color to default, which is to match the foreground color|
|73  |VerticalAlignSuperScript|Adjusts the baseline of the text so that it renders as superscript {{since('20221119-145034-49b9839f', inline=True)}}|
|74  |VerticalAlignSubScript|Adjusts the baseline of the text so that it renders as subscript {{since('20221119-145034-49b9839f', inline=True)}}|
|75  |VerticalAlignBaseLine|Reset the baseline of the text to normal {{since('20221119-145034-49b9839f', inline=True)}}|
|90  |ForegroundBrightBlack|Sets the foreground color to Bright Black, which is palette index 8|
|91  |ForegroundBrightRed|Sets the foreground color to Bright Red, which is palette index 9|
|92  |ForegroundBrightGreen|Sets the foreground color to Bright Green, which is palette index 10|
|93  |ForegroundBrightYellow|Sets the foreground color to Bright Yellow, which is palette index 11|
|94  |ForegroundBrightBlue|Sets the foreground color to Bright Blue, which is palette index 12|
|95  |ForegroundBrightMagenta|Sets the foreground color to Bright Magenta, which is palette index 13|
|96  |ForegroundBrightCyan|Sets the foreground color to Bright Cyan, which is palette index 14|
|97  |ForegroundBrightWhite|Sets the foreground color to Bright White, which is palette index 15|
|100  |BackgroundBrightBlack|Sets the background color to Bright Black, which is palette index 8|
|101  |BackgroundBrightRed|Sets the background color to Bright Red, which is palette index 9|
|102  |BackgroundBrightGreen|Sets the background color to Bright Green, which is palette index 10|
|103  |BackgroundBrightYellow|Sets the background color to Bright Yellow, which is palette index 11|
|104  |BackgroundBrightBlue|Sets the background color to Bright Blue, which is palette index 12|
|105  |BackgroundBrightMagenta|Sets the background color to Bright Magenta, which is palette index 13|
|106  |BackgroundBrightCyan|Sets the background color to Bright Cyan, which is palette index 14|
|107  |BackgroundBrightWhite|Sets the background color to Bright White, which is palette index 15|

There are a handful of additional SGR codes that allow setting extended colors;
unlike the codes above, which are activated by a single numeric parameter out
of SGR sequence, these the extended color codes require multiple parameters.
The canonical representation of these sequences is to have the multiple
parameters be separated by colons (`:`), but for compatibility reasons WezTerm
also accepts an ambiguous semicolon (`;`) separated variation.  The colon form
is unambiguous and should be preferred; the semicolon form should not be used
by new applications and is not documented here in the interest of avoiding
accidental new implementations.

##### CSI 38:5 - foreground color palette index

This sequence will set the *foreground color* to the specified palette INDEX,
which can be a decimal number in the range `0-255`.

```
CSI 38 : 5 : INDEX m
```

##### CSI 48:5 - background color palette index

This sequence will set the *background color* to the specified palette INDEX,
which can be a decimal number in the range `0-255`.

```
CSI 48 : 5 : INDEX m
```

##### CSI 58:5 - underline color palette index

This sequence will set the *underline color* to the specified palette INDEX,
which can be a decimal number in the range `0-255`.

```
CSI 58 : 5 : INDEX m
```

##### CSI 38:2 - foreground color: RGB

This sequence will set the *foreground color* to an arbitrary color in RGB
colorspace.  The `R`, `G` and `B` symbols below are decimal numbers in the
range `0-255`.  Note that after the `2` parameter two colons are present; its
really an omitted *colorspace ID* parameter but that nature of that parameter
is not specified in the accompanying ITU T.416 specification and is ignored by
`WezTerm` and most (all?) other terminal emulators:

```
CSI 38 : 2 : : R : G : B m
```

(*Since 20210814-124438-54e29167*) For the sake of compatibility with some other
terminal emulators this additional form is also supported where the colorspace
ID argument is not specified:

```
CSI 38 : 2 : R : G : B m
```

##### CSI 38:6 - foreground color: RGBA

{{since('20220807-113146-c2fee766')}}

This is a wezterm extension: wezterm considers color mode `6` as RGBA,
allowing you to specify the alpha channel in addition to the RGB channels.

```
CSI 38 : 6 : : R : G : B : A m
```

##### CSI 48:2 - background color: RGB

This sequence will set the *background color* to an arbitrary color in RGB colorspace.
The `R`, `G` and `B` symbols below are decimal numbers in the range `0-255`:

```
CSI 48 : 2 : : R : G : B m
```

(*Since 20210814-124438-54e29167*) For the sake of compatibility with some other
terminal emulators this additional form is also supported where the colorspace
ID argument is not specified:

```
CSI 48 : 2 : R : G : B m
```

##### CSI 48:6 - background color: RGBA

{{since('20220807-113146-c2fee766')}}

This is a wezterm extension: wezterm considers color mode `6` as RGBA,
allowing you to specify the alpha channel in addition to the RGB channels.

```
CSI 48 : 6 : : R : G : B : A m
```

##### CSI 58:2 - underline color: RGB

This sequence will set the *underline color* to an arbitrary color in RGB colorspace.
The `R`, `G` and `B` symbols below are decimal numbers in the range `0-255`:

```
CSI 58 : 2 : : R : G : B m
```

(*Since 20210814-124438-54e29167*) For the sake of compatibility with some other
terminal emulators this additional form is also supported where the colorspace
ID argument is not specified:

```
CSI 58 : 2 : R : G : B m
```

##### CSI 58:6 - underline color: RGBA

{{since('20220807-113146-c2fee766')}}

This is a wezterm extension: wezterm considers color mode `6` as RGBA,
allowing you to specify the alpha channel in addition to the RGB channels.

```
CSI 58 : 6 : : R : G : B : A m
```

#### Cursor Movement

These sequences have the form `CSI Ps X`, where the parameter `Ps` defaults to
`1` when it is omitted.  Movement is constrained by the current top/bottom and
left/right margins.

|Seq|Name|Action|
|---|----|------|
|CSI Ps A|[CUU](https://vt100.net/docs/vt510-rm/CUU.html)|Move the cursor up `Ps` rows; stops at the top margin|
|CSI Ps B|[CUD](https://vt100.net/docs/vt510-rm/CUD.html)|Move the cursor down `Ps` rows; stops at the bottom margin|
|CSI Ps C|[CUF](https://vt100.net/docs/vt510-rm/CUF.html)|Move the cursor right `Ps` columns; stops at the right margin|
|CSI Ps D|[CUB](https://vt100.net/docs/vt510-rm/CUB.html)|Move the cursor left `Ps` columns.  Implemented as `Ps` Backspaces to match xterm (see [#1273](https://github.com/wezterm/wezterm/issues/1273))|
|CSI Ps E|[CNL](https://vt100.net/docs/vt510-rm/CNL.html)|Move down `Ps` lines and to the left margin|
|CSI Ps F|[CPL](https://vt100.net/docs/vt510-rm/CPL.html)|Move up `Ps` lines and to the left margin|
|CSI Ps G|[CHA](https://vt100.net/docs/vt510-rm/CHA.html)|Move to column `Ps` on the current line|
|CSI Ps ; Ps H|[CUP](https://vt100.net/docs/vt510-rm/CUP.html)|Move to the given line;column, honoring origin mode|
|CSI Ps ; Ps f|[HVP](https://vt100.net/docs/vt510-rm/HVP.html)|Horizontal and Vertical Position; identical to CUP|
|CSI Ps I|[CHT](https://vt100.net/docs/vt510-rm/CHT.html)|Advance the cursor to the `Ps`-th following tab stop|
|CSI Ps Z|[CBT](https://vt100.net/docs/vt510-rm/CBT.html)|Move the cursor back to the `Ps`-th preceding tab stop|
|CSI Ps &#96;|[HPA](https://vt100.net/docs/vt510-rm/HPA.html)|Move to absolute column position `Ps` (the final byte is a backtick)|
|CSI Ps a|[HPR](https://vt100.net/docs/vt510-rm/HPR.html)|Move the cursor right `Ps` columns (relative)|
|CSI Ps j|HPB|Move the cursor left `Ps` columns (relative)|
|CSI Ps d|[VPA](https://vt100.net/docs/vt510-rm/VPA.html)|Move to absolute line position `Ps`; the column is unchanged|
|CSI Ps e|[VPR](https://vt100.net/docs/vt510-rm/VPR.html)|Move the cursor down `Ps` lines (relative)|
|CSI Ps k|VPB|Move the cursor up `Ps` lines (relative)|
|CSI Ps g|[TBC](https://vt100.net/docs/vt510-rm/TBC.html)|Clear tab stops: `0` (default) clears the stop at the cursor column, `3` clears all tab stops|
|CSI Ps W|CTC|Cursor Tabulation Control; recognized but has no effect|
|CSI Ps Y|CVT|Cursor Line Tabulation; recognized but has no effect|
|CSI 6 n|[CPR](https://vt100.net/docs/vt510-rm/CPR.html)|Cursor Position Report (DSR 6); replies with the cursor position as `CSI line ; col R`, honoring origin mode|
|CSI Pt ; Pb r|[DECSTBM](https://vt100.net/docs/vt510-rm/DECSTBM.html)|Set the top and bottom margins (the vertical scrolling region) and home the cursor|
|CSI Pl ; Pr s|[DECSLRM](https://vt100.net/docs/vt510-rm/DECSLRM.html)|Set the left and right margins **when DECLRMM (private mode 69) is enabled**; otherwise the `s` byte is SCP, below|
|CSI s|SCP / DECSLRM|Save the cursor position and pen (like [DECSC](https://vt100.net/docs/vt510-rm/DECSC.html)); or act as DECSLRM when DECLRMM is enabled|
|CSI u|RCP|Restore the cursor position and pen saved by SCP (like [DECRC](https://vt100.net/docs/vt510-rm/DECRC.html))|
|CSI Ps SP q|[DECSCUSR](https://vt100.net/docs/vt510-rm/DECSCUSR.html)|Set the cursor style (`SP` is a literal space): `0`/`1` blinking block, `2` steady block, `3` blinking underline, `4` steady underline, `5` blinking bar, `6` steady bar|

#### Editing Functions

These sequences have the form `CSI Ps X`, where the parameter `Ps` defaults to
`1` when it is omitted.

|Seq|Name|Action|
|---|----|------|
|CSI Ps @|[ICH](https://vt100.net/docs/vt510-rm/ICH.html)|Insert `Ps` blank cells at the cursor; existing cells shift right and any past the right margin are lost|
|CSI Ps P|[DCH](https://vt100.net/docs/vt510-rm/DCH.html)|Delete `Ps` characters at the cursor; the rest of the line shifts left within the margins|
|CSI Ps X|[ECH](https://vt100.net/docs/vt510-rm/ECH.html)|Erase (blank) `Ps` cells starting at the cursor; the cursor does not move|
|CSI Ps L|[IL](https://vt100.net/docs/vt510-rm/IL.html)|Insert `Ps` blank lines at the cursor row within the scrolling region|
|CSI Ps M|[DL](https://vt100.net/docs/vt510-rm/DL.html)|Delete `Ps` lines at the cursor row within the scrolling region|
|CSI Ps K|[EL](https://vt100.net/docs/vt510-rm/EL.html)|Erase in line: `0` cursor to end of line, `1` start of line to cursor, `2` the whole line|
|CSI Ps J|[ED](https://vt100.net/docs/vt510-rm/ED.html)|Erase in display: `0` cursor to end, `1` start to cursor, `2` the whole display, `3` the scrollback (xterm extension)|
|CSI Ps S|[SU](https://vt100.net/docs/vt510-rm/SU.html)|Scroll the scrolling region up `Ps` lines|
|CSI Ps T|[SD](https://vt100.net/docs/vt510-rm/SD.html)|Scroll the scrolling region down `Ps` lines|
|CSI Ps b|REP|Repeat the preceding printed character `Ps` times|

#### Mode Functions

Modes are enabled with Set Mode (`CSI Ps h`) and disabled with Reset Mode
(`CSI Ps l`).  The DEC private modes below use the `?` prefix form: DECSET
(`CSI ? Ps h`) and DECRST (`CSI ? Ps l`).  The current state of a mode can be
queried with DECRQM (`CSI Ps $ p`, or `CSI ? Ps $ p` for private modes), which
replies with a DECRPM report of the form `CSI ? Ps ; Pstate $ y`.

ANSI modes (`CSI Ps h` / `CSI Ps l`):

|Code|Name|Action|
|----|----|------|
|2|KAM|Keyboard Action Mode; recognized but has no effect|
|4|IRM|Insert/Replace Mode; when set, printed cells are inserted rather than overwriting|
|8|—|Enable or disable bidirectional text support|
|12|SRM|Send/Receive (local echo); recognized but has no effect|
|20|LNM|Automatic Newline; when set, `LF`, `VT` and `FF` also perform a carriage return|
|25|—|Show or hide the cursor (a Microsoft terminal variant of DECTCEM)|

DEC private modes (`CSI ? Ps h` / `CSI ? Ps l`):

|Code|Name|Action|
|----|----|------|
|1|DECCKM|Application cursor keys|
|2|DECANM|Select VT52 (reset) or ANSI (set) behavior|
|3|DECCOLM|132-column mode is not supported; either value resets the margins, homes the cursor and erases the display|
|4|DECSCLM|Smooth scroll; recognized but has no effect|
|5|DECSCNM|Reverse video across the whole screen|
|6|DECOM|Origin mode; cursor addressing becomes relative to the scrolling region|
|7|DECAWM|Auto-wrap mode|
|8|DECARM|Auto-repeat; recognized but has no effect|
|12|att610|Start blinking cursor; recognized but has no effect|
|25|DECTCEM|Show or hide the cursor|
|45|—|Reverse-wraparound mode (see the Backspace `C0` control above)|
|47|—|Switch to the alternate screen (without clearing it or saving the cursor)|
|69|DECLRMM|Left/right margin mode; enables DECSLRM|
|80|DECSDM|Sixel display mode|
|1036|—|`metaSendsEscape`; recognized but has no effect|
|1039|—|`altSendsEscape`; recognized but has no effect|
|1047|—|Switch to the alternate screen; resetting it first erases the alternate screen|
|1048|—|Save (set) or restore (reset) the cursor, like DECSC / DECRC|
|1049|—|Save the cursor and switch to a cleared alternate screen; reverse this on reset|
|1070|—|Use private color registers for each sixel/ReGIS graphic|
|2004|—|Bracketed paste mode; pasted text is bracketed with `CSI 200 ~` and `CSI 201 ~`|
|2026|—|Synchronized output; see the note below|
|2027|—|Grapheme clustering; permanently enabled (DECRQM reports it as permanently set)|
|7727|—|minTTY application-escape-key mode; recognized but has no effect|
|8452|—|Position the cursor to the right of a sixel graphic after drawing it|
|9001|—|Win32 input mode (Windows Terminal)|

{{since('20210814-124438-54e29167')}}

WezTerm supports [Synchronized Rendering](https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036).
DECSET 2026 is set to batch (hold) rendering until DECSET 2026 is reset to flush the queued screen data.

Related notes:

* `CSI ? Ps s` / `CSI ? Ps r` (save/restore a DEC private mode) are recognized
  but not implemented.
* XTMODKEYS (`CSI > Ps ; Ps m`) is accepted, but only resource `4`
  (`modifyOtherKeys`) has an effect.
* The mouse-tracking private modes are documented separately under
  [Mouse Functions](#mouse-functions), below.

#### Mouse Functions

These DEC private modes (set with `CSI ? Ps h`, reset with `CSI ? Ps l`) control
mouse and focus reporting.  The *tracking* modes select which events are
reported and the *encoding* modes select how the coordinates are encoded; the
two groups are independent and combine.

Mouse tracking modes:

|Code|Name|Action|
|----|----|------|
|1000|Normal tracking|Report mouse button press and release|
|1001|Highlight tracking|Recognized but has no effect|
|1002|Button-event tracking|Report press/release plus motion while a button is held (drag)|
|1003|Any-event tracking|Report all pointer motion, with buttons up or down|
|1004|Focus tracking|Report window focus changes as `CSI I` (focus in) / `CSI O` (focus out)|

Mouse encoding modes:

|Code|Name|Action|
|----|----|------|
|1005|UTF-8|Encode the coordinates as UTF-8, extending their range beyond the default byte encoding|
|1006|SGR|Report events as `CSI < b ; x ; y M` (press/motion) and `CSI < b ; x ; y m` (release), in character cells|
|1016|SGR-Pixels|As SGR, but the coordinates are reported in pixels rather than cells|

With no encoding mode set, WezTerm uses the default X10 encoding
(`CSI M Cb Cx Cy`); resetting an encoding mode returns to X10.  There is
deliberately no urxvt (mode 1015) encoding.

#### Device Functions

|Seq|Name|Action|
|---|----|------|
|CSI c|[DA1](https://vt100.net/docs/vt510-rm/DA1.html)|Primary Device Attributes; replies `CSI ? 65 ; 4 ; 6 ; 18 ; 22 ; 52 c` (VT500 with sixel, selective erase, windowing extensions, ANSI color and clipboard access)|
|CSI > c|[DA2](https://vt100.net/docs/vt510-rm/DA2.html)|Secondary Device Attributes; replies `CSI > 1 ; 277 ; 0 c` (VT220 class; firmware `277` advertises SGR mouse)|
|CSI = c|[DA3](https://vt100.net/docs/vt510-rm/DA3.html)|Tertiary Device Attributes; replies `DCS ! | 00000000 ST`|
|CSI 5 n|[DSR](https://vt100.net/docs/vt510-rm/DSR-OS.html)|Device Status Report; replies `CSI 0 n` (terminal OK)|
|CSI ! p|[DECSTR](https://vt100.net/docs/vt510-rm/DECSTR.html)|Soft terminal reset: resets the pen, insert mode, origin mode, margins, saved cursors, character sets and related state without erasing the screen|
|CSI > q|XTVERSION|Report the terminal name and version; replies `DCS > | <program> <version> ST`|
|CSI Ps x|DECREQTPARM|Request Terminal Parameters; replies `CSI <Ps+2> ; 1 ; 1 ; 128 ; 128 ; 1 ; 0 x`|
|CSI ? Pi ; Pa ; Pv S|XTSMGRAPHICS|Query/reset the number of color registers (item `1` → 65536) and the sixel/ReGIS geometry (items `2` and `3` → the pixel dimensions); replies `CSI ? ... S`|

The Cursor Position Report request (`CSI 6 n`) is listed under
[Cursor Movement](#cursor-movement), above.

#### Window Functions

Window manipulation (XTWINOPS) uses `CSI Ps ; Ps ; Ps t`.  WezTerm answers the
reporting variants and ignores the ones that would let an application move,
resize, raise or lower the window.

|Seq|Name|Action|
|---|----|------|
|CSI 14 t|Report text area size (pixels)|Replies `CSI 4 ; height ; width t`|
|CSI 16 t|Report cell size (pixels)|Replies `CSI 6 ; height ; width t`|
|CSI 18 t|Report text area size (cells)|Replies `CSI 8 ; rows ; cols t`|
|CSI 21 t|Report window title|Replies with the title via OSC `l`; gated on `enable_title_reporting` (disabled by default)|
|CSI 1 t / CSI 2 t|De-iconify / iconify|Recognized but has no effect|
|CSI 8 ; h ; w t|Resize in cells|Ignored; applications are not permitted to resize the window|
|CSI 22 ; Ps t / CSI 23 ; Ps t|Push / pop title stack|Recognized but not implemented|
|CSI 3/4/5/6/7/9/10/11/13/15/19/20 ... t|Move / resize / raise / lower / report window|Recognized but not implemented|
|CSI Pid ; Pt ; Pl ; Pb ; Pr * y|DECRQCRA|Checksum of a rectangular area; replies `DCS Pid ! ~ <hex> ST`.  Gated on `enable_checksum_rectangular_area` (used by esctest).  Note the `* y` final bytes|

### DCS - Device Control String

The `C1` `DCS` escape places the terminal parser into a device control mode until the `C1` `ST` is encountered.

In the table below, `DCS` can be either the 7-bit representation (`ESC P`) or the 8-bit codepoint (`0x90`).

|Seq     | Name  | Description         |
|--------|-------|---------------------|
|DCS $ q " p ST | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSCL](https://vt100.net/docs/vt510-rm/DECSCL.html) | Request Conformance Level; Reports the conformance level |
|DCS $ q r ST   | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSTBM](https://vt100.net/docs/vt510-rm/DECSTBM.html) | Request top and bottom margin report; Reports the margins |
|DCS $ q s ST   | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSLRM](https://vt100.net/docs/vt510-rm/DECSLRM.html) | Request left and right margin report; Reports the margins |
|DCS \[PARAMS\] q \[DATA\] ST | Sixel Graphic Data | Decodes [Sixel graphic data](https://vt100.net/docs/vt3xx-gp/chapter14.html) and apply the image to the terminal model. Support is preliminary and incomplete; see [this issue](https://github.com/wezterm/wezterm/issues/217) for status. |
|DCS + q \[NAMES\] ST | XTGETTCAP | Request Termcap/Terminfo String. Replies with the hex-encoded value for each hex-encoded capability name. `TN`/`name` reports the terminal program, `Co`/`colors` reports `256`, `RGB` reports `8/8/8`; other names are looked up in the terminfo database. Unknown names get an invalid `DCS 0 + r ... ST` reply. |
|DCS 1000 p | tmux control mode | Bridges tmux into the WezTerm multiplexer.  Currently incomplete, see [this issue](https://github.com/wezterm/wezterm/issues/336) for status. |

A DECRQSS (`DCS $ q ... ST`) request for anything other than the three settings
listed above is answered with the invalid reply `DCS 0 $ r ST`.

### Operating System Command Sequences

Operating System Command (OSC) sequences are introduced via `ESC ]` followed by
a numeric code and typically have parameters delimited by `;`.  OSC sequences
are canonically delimited by the `ST` (String Terminator) sequence, but WezTerm
also accepts delimiting them with the `BEL` control.

The table below is keyed by the OSC code.

|OSC|Description|Action|Example|
|---|-----------|------|-------|
|0  |Set Icon Name and Window Title | Clears Icon Name, sets Window Title. | `\x1b]0;title\x1b\\` |
|1  |Set Icon Name | Sets Icon Name, which is used as the Tab title when it is non-empty | `\x1b]1;tab-title\x1b\\` |
|2  |Set Window Title | Set Window Title | `\x1b]2;window-title\x1b\\` |
|3  |Set X11 Window Property | Ignored | |
|4  |Change/Query Color Number | Set or query color palette entries 0-255. | query color number 1: `\x1b]4;1;?\x1b\\` <br/> Set color number 2: `\x1b]4;2;#cccccc\x1b\\` |
|5  |Change/Query Special Color Number | Ignored | |
|6  |iTerm2 Change Title Tab Color | Ignored | |
|7  |Set Current Working Directory | [See Shell Integration](shell-integration.md#osc-7-escape-sequence-to-set-the-working-directory) ||
|8  |Set Hyperlink | [See Explicit Hyperlinks](hyperlinks.md#explicit-hyperlinks) | |
|9  |iTerm2 Show System Notification | Show a "toast" notification | `printf "\e]9;%s\e\\" "hello there"` |
|9;4 |ConEmu Progress | Reports task progress to the tab/window (percentage, error, indeterminate or none) | `printf "\e]9;4;1;50\e\\"` |
|10 |Set Default Text Foreground Color| | `\x1b]10;#ff0000\x1b\\`.<br/> Also supports RGBA in nightly builds: `printf "\e]10;rgba(127,127,127,0.4)\x07"` |
|11 |Set Default Text Background Color| | `\x1b]11;#0000ff\x1b\\`.<br/> Also supports RGBA in nightly builds: `printf "\e]11;rgba:efff/ecff/f4ff/d000\x07"` |
|12 |Set Text Cursor Color| | `\x1b]12;#00ff00\x1b\\`.<br/> Also supports RGBA in nightly builds. |
|13 |Set Mouse Pointer Foreground Color | Recognized but has no effect | |
|14 |Set Mouse Pointer Background Color | Recognized but has no effect | |
|15 |Set Tektronix Foreground Color | Recognized but has no effect | |
|16 |Set Tektronix Background Color | Recognized but has no effect | |
|17 |Set Selection Background Color | Sets the background color used for selected text | `\x1b]17;#cccccc\x1b\\` |
|18 |Set Tektronix Cursor Color | Recognized but has no effect | |
|19 |Set Selection Foreground Color | Sets the foreground color used for selected text | `\x1b]19;#000000\x1b\\` |
|46 |Set Log File Name | Ignored | |
|50 |Set Font | Ignored | |
|51 |Emacs Shell | Ignored | |
|52 |Manipulate clipboard | Requests to query the clipboard are ignored. Allows setting or clearing the clipboard | |
|104|ResetColors | Reset color palette entries to their default values | |
|105|Reset Special Color | Ignored | |
|110|Reset Default Text Foreground Color | Reset the default text foreground color to the configured value | |
|111|Reset Default Text Background Color | Reset the default text background color to the configured value | |
|112|Reset Text Cursor Color | Reset the text cursor color to the configured value | |
|113|Reset Mouse Pointer Foreground Color | Recognized but has no effect | |
|114|Reset Mouse Pointer Background Color | Recognized but has no effect | |
|115|Reset Tektronix Foreground Color | Recognized but has no effect | |
|116|Reset Tektronix Background Color | Recognized but has no effect | |
|117|Reset Selection Background Color | Reset the selection background color | |
|118|Reset Tektronix Cursor Color | Recognized but has no effect | |
|119|Reset Selection Foreground Color | Reset the selection foreground color | |
|133|FinalTerm semantic escapes| Informs the terminal about Input, Output and Prompt regions on the display | [See Shell Integration](shell-integration.md) |
|777|Call rxvt extension| Only the notify extension is supported; it shows a "toast" notification | `printf "\e]777;notify;%s;%s\e\\" "title" "body"` |
|1337 |iTerm2 proprietary escapes | `File=` displays images inline; `SetUserVar=NAME=BASE64` sets a user var (exposed to the config/Lua as a user var); `UnicodeVersion=` sets, pushes or pops the active Unicode width version; `RequestCellSize` replies with the per-cell pixel size. Other iTerm2 subcommands are parsed but ignored. | [See iTerm Image Protocol](imgcat.md) |
|L  |Set Icon Name (Sun) | Same as OSC 1 | `\x1b]Ltab-title\x1b\\` |
|l  |Set Window Title (Sun) | Same as OSC 2 | `\x1b]lwindow-title\x1b\\` |

# Additional Resources

* [xterm's escape sequences](http://invisible-island.net/xterm/ctlseqs/ctlseqs.txt)
* [iTerm2's escape sequences](https://iterm2.com/documentation-escape-codes.html)
* [kitty's escape sequences](https://sw.kovidgoyal.net/kitty/protocol-extensions.html)
* [Terminology's escape sequences](https://github.com/billiob/terminology#extended-escapes-for-terminology-only)
* [This Google spreadsheet](https://docs.google.com/spreadsheets/d/19W-lXWS9jYwqCK-LwgYo31GucPPxYVld_hVEcfpNpXg/edit?usp=sharing)
  aims to document all the known escape sequences.
* [Wikipedia's ANSI escape code page](https://en.wikipedia.org/wiki/ANSI_escape_code)
