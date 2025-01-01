---
tags:
  - quick_select
---
# `quick_select_alphabet`

{{since('20210502-130208-bff6815d')}}

Specify the alphabet used to produce labels for the items
matched in [quick select mode](../../../quickselect.md).

The default alphabet is `"asdfqwerzxcvjklmiuopghtybn"` which
means that the first matching item from the bottom is labelled
with an `a`, the second with `s` and so forth; these are easily
accessible characters in a `qwerty` keyboard layout.

|Keyboard Layout|Suggested Alphabet|
|---------------|------------------|
|`qwerty`       |`"asdfqwerzxcvjklmiuopghtybn"` (this is the default)|
|`qwertz`       |`"asdfqweryxcvjkluiopmghtzbn"`|
|`azerty`       |`"qsdfazerwxcvjklmuiopghtybn"`|
|`dvorak`       |`"aoeuqjkxpyhtnsgcrlmwvzfidb"`|
|`colemak`      |`"arstqwfpzxcvneioluymdhgjbk"`|

The suggested alphabet in the above table uses the left 4 fingers on the home row, top row, bottom
row, then the right 4 fingers on the home raw, top row, bottom row, followed by the characters in
the middle of the keyboard that may be harder to reach.
