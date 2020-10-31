# `pane:paste(text)`

*Since: 20201031-154415-9614e117*

Sends the supplied `text` string to the input of the pane as if it
were pasted from the clipboard, except that the clipboard is not involved.

If the terminal attached to the pane is set to bracketed paste mode then
the text will be sent as a bracketed paste.

Otherwise the string will be streamed into the input in chunks of
approximately 1KB each.
