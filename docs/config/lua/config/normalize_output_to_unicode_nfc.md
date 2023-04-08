---
tags:
  - unicode
---
# `normalize_output_to_unicode_nfc = false`

{{since('20221119-145034-49b9839f')}}

When set to true, contiguous runs codepoints output to the terminal
are [normalized](http://www.unicode.org/faq/normalization.html) to [Unicode
Normalization Form C (NFC)](https://www.unicode.org/reports/tr15/#Norm_Forms).

This can improve the display of text and in the terminal when portions of the
output are in other normalization forms, particularly with Korean text where a
given glyph can be comprised of several codepoints.

However, depending on the application running inside the terminal, enabling
this option may introduce discrepancies in the understanding of text
positioning: while it may fix some display glitches for some applications, it
may trade them for other glitches.

As such, you should consider this configuration setting to be an imperfect
option!

This option defaults to `false` as it introduces some additional text
processing that is not necessary for most users.

