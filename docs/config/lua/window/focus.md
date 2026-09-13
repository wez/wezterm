# window:focus()

{{since('20230320-124340-559cb7b0')}}

Attempts to focus and activate the window.

|OS             |Supported?|
|---------------|------------------------|
|macOS          |Yes                     |
|Windows        |Yes                     |
|X11            |Yes                     |
|Wayland        |Yes*                    |

\* On Wayland the request goes through `xdg-activation-v1` and the compositor
decides. It is honored when another WezTerm window already has focus.
Otherwise the compositor typically refuses it and marks the window as
requesting attention instead, however the desktop chooses to show that.
Nothing happens if the compositor does not implement the protocol.
