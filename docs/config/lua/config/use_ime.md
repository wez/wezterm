---
tags:
  - keys
---
# `use_ime`

Controls whether the Input Method Editor (IME) will be used to process keyboard
input.  The IME is useful for inputting kanji or other text that is not
natively supported by the attached keyboard hardware.

IME support is a platform dependent feature

|Platform  |Supported since|  Notes|
|----------|---------------|-------|
|Windows   |Forever        |Always enabled, cannot be disabled|
|macOS     |20200113-214446-bb6251f|defaults to enabled starting in 20220319-142410-0fcdea07. Earlier versions had problems with key repeat when enabled|
|X11       |20211204-082213-a66c61ee9|[XIM](https://en.wikipedia.org/wiki/X_Input_Method) based. Your system needs to have a running input method engine (such as ibus or fcitx) that support the XIM protocol in order for wezterm to use it.|
|Wayland   |20220807-113146-c2fee766|Your compositor must support `zwp_text_input_v3`|

You can control whether the IME is enabled in your configuration file:

```lua
config.use_ime = false
```

Changing `use_ime` usually requires re-launching WezTerm to take full effect.

{{since('20200620-160318-e00b076c')}}

The default for `use_ime` is false.  The default in earlier releases was `true`.

{{since('20220101-133340-7edc5b5a')}}

The default for X11 systems is now `true`.  Please ensure that the `XMODIFIERS`
environment variable or the new [xim_im_name](xim_im_name.md) configuration
option is set appropriately before wezterm is launched!  For
example, Gnome users will probably want to set `XMODIFIERS=@im=ibus`.

{{since('20220319-142410-0fcdea07')}}

The default for all systems is now `true`
