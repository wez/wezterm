---
tags:
  - osc
  - clipboard
---

# `enable_osc52_clipboard_reading = false`

{{since('nightly')}}

When set to `true`, the terminal will allow access to the system clipboard by
terminal applications via `OSC 52` [escape sequence](../../../shell-integration.md#osc-52-clipboard-paste).

The default for this option is `false`.

Note that it is not recommended to enable this option due to serious security
implications.

### Security Concerns

Clipboards are often used to store sensitive information, and granting any
terminal application (especially from remote machines) access to it poses a
security risk. A malicious server could spam the terminal with OSC 52 paste
sequences to monitor whatever you have on the clipboard, which may occasionally
contain sensitive data.

Setting clipboard data that contains escape sequences or malicious commands and
reading it back could allow an attacker to inject harmful characters into the
input stream. Although, the risk is somewhat mitigated as the pasted text is
encoded in BASE64.
