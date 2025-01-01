---
tags:
  - keys
  - appearance
---
# `ime_preedit_rendering`

{{since('20220624-141144-bd1b7c5d')}}

Control IME preedit rendering.
IME preedit is an area that is used to display the string being preedited in IME.
WezTerm supports the following IME preedit rendering.

* `"Builtin"` - (Default) IME preedit is rendered by WezTerm itself

  "Builtin" rendering provides good look and feel for many IMEs,
  rendering the text using the same font as the terminal and
  works in concert with features like [window:composition_status()](../window/composition_status.md).

* `"System"` - IME preedit is rendered by system

  "Builtin" rendering may truncate displaying of IME preedit
  at the end of window if IME preedit is very long
  because the rendering does not allow the IME preedit to overflow the window
  and does not wrap IME preedit to the next line.
  "System" rendering can be useful
  to avoid the truncated displaying of IME preedit
  but has a worse look and feel compared to "Builtin" rendering.

You can control IME preedit rendering in your configuration file:

```lua
config.ime_preedit_rendering = 'System'
```

Otherwise, the default is `"Builtin"`.

Note:

* Changing `ime_preedit_rendering` usually requires re-launching WezTerm to take full effect.
* In macOS, `ime_preedit_rendering` has effected nothing yet.
  IME preedit is always rendered by WezTerm itself.
