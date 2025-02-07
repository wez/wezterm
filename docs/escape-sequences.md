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
|ESC (0 |        | DEC Line Drawing character set | Translate characters `j-x` to line drawing glyphs |
|ESC (B |        | US ASCII character set | Disables DEC Line Drawing character translation |
|ESC #8 | [DECALN](https://vt100.net/docs/vt510-rm/DECALN.html) | Screen Alignment Display | Fills the display with `E` characters for diagnostic/test purposes (for vttest) |

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

#### Editing Functions

#### Mode Functions

{{since('20210814-124438-54e29167')}}

WezTerm supports [Synchronized Rendering](https://gist.github.com/christianparpart/d8a62cc1ab659194337d73e399004036).
DECSET 2026 is set to batch (hold) rendering until DECSET 2026 is reset to flush the queued screen data.

#### Device Functions

#### Window Functions

### DCS - Device Control String

The `C1` `DCS` escape places the terminal parser into a device control mode until the `C1` `ST` is encountered.

In the table below, `DCS` can be either the 7-bit representation (`ESC P`) or the 8-bit codepoint (`0x90`).

|Seq     | Name  | Description         |
|--------|-------|---------------------|
|DCS $ q " p ST | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSCL](https://vt100.net/docs/vt510-rm/DECSCL.html) | Request Conformance Level; Reports the conformance level |
|DCS $ q r ST   | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSTBM](https://vt100.net/docs/vt510-rm/DECSTBM.html) | Request top and bottom margin report; Reports the margins |
|DCS $ q s ST   | [DECRQSS](https://vt100.net/docs/vt510-rm/DECRQSS.html) for [DECSLRM](https://vt100.net/docs/vt510-rm/DECSLRM.html) | Request left and right margin report; Reports the margins |
|DCS \[PARAMS\] q \[DATA\] ST | Sixel Graphic Data | Decodes [Sixel graphic data](https://vt100.net/docs/vt3xx-gp/chapter14.html) and apply the image to the terminal model. Support is preliminary and incomplete; see [this issue](https://github.com/wezterm/wezterm/issues/217) for status. |
|DCS 1000 q | tmux control mode | Bridges tmux into the WezTerm multiplexer.  Currently incomplete, see [this issue](https://github.com/wezterm/wezterm/issues/336) for status. |

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
|10 |Set Default Text Foreground Color| | `\x1b]10;#ff0000\x1b\\`.<br/> Also supports RGBA in nightly builds: `printf "\e]10;rgba(127,127,127,0.4)\x07"` |
|11 |Set Default Text Background Color| | `\x1b]11;#0000ff\x1b\\`.<br/> Also supports RGBA in nightly builds: `printf "\e]11;rgba:efff/ecff/f4ff/d000\x07"` |
|12 |Set Text Cursor Color| | `\x1b]12;#00ff00\x1b\\`.<br/> Also supports RGBA in nightly builds. |
|52 |Manipulate clipboard | Requests to query the clipboard are ignored. Allows setting or clearing the clipboard | |
|104|ResetColors | Reset color palette entries to their default values | |
|133|FinalTerm semantic escapes| Informs the terminal about Input, Output and Prompt regions on the display | [See Shell Integration](shell-integration.md) |
|777|Call rxvt extension| Only the notify extension is supported; it shows a "toast" notification | `printf "\e]777;notify;%s;%s\e\\" "title" "body"` |
|1337 |iTerm2 File Upload Protocol | Allows displaying images inline | [See iTerm Image Protocol](imgcat.md) |
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
