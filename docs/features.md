---
hide:
  - toc
---

## Available Features

* Runs on Linux, macOS, Windows 10, FreeBSD and NetBSD
* [Multiplex terminal panes, tabs and windows on local and remote hosts, with native mouse and scrollback](multiplexing.md)
* <a href="https://github.com/tonsky/FiraCode#fira-code-monospaced-font-with-programming-ligatures">Ligatures</a>, Color Emoji and font fallback, with true color and [dynamic color schemes](config/appearance.md).
* [Hyperlinks](hyperlinks.md)
* [Searchable Scrollback](scrollback.md) (use mouse wheel and `Shift-PageUp` and `Shift PageDown` to navigate, Ctrl-Shift-F to activate search mode)
* xterm style selection of text with mouse; paste selection via `Shift-Insert` (bracketed paste is supported!)
* SGR style mouse reporting (works in vim and tmux)
* Render underline, double-underline, italic, bold, strikethrough (most other terminal emulators do not support as many render attributes)
* Configuration via a [configuration file](config/files.md) with hot reloading
* Multiple Windows (Hotkey: `Super-N`)
* Splits/Panes (Split horizontally/vertically: `Ctrl-Shift-Alt-%` and `Ctrl-Shift-Alt-"`, move between panes: `Ctrl-Shift-ArrowKey`)
* Tabs (Hotkey: `Super-T`, next/prev: `Super-Shift-[` and `Super-Shift-]`, go-to: `Super-[1-9]`)
  <video width="80%" controls src="screenshots/wezterm-tabs.mp4" loop></video>
* [SSH client with native tabs](ssh.md)
* [Connect to serial ports for embedded/Arduino work](serial.md)
* Connect to a local multiplexer server over unix domain sockets
* Connect to a remote multiplexer using SSH or TLS over TCP/IP
* iTerm2 compatible image protocol support, and built-in [imgcat command](imgcat.md)
* Kitty graphics support
* Sixel graphics support (experimental: starting in `20200620-160318-e00b076c`)
