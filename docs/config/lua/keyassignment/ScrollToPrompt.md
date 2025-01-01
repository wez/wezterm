# `ScrollToPrompt`

{{since('20210203-095643-70a364eb')}}

This action operates on Semantic Zones defined by applications that use [OSC
133 Semantic Prompt Escapes](https://gitlab.freedesktop.org/Per_Bothner/specifications/blob/master/proposals/semantic-prompts.md) and requires configuring your shell to emit those sequences.

OSC 133 escapes allow marking regions of output as `Output` (from the commands
that you run), `Input` (that you type) and `Prompt` ("chrome" from your shell).

This action allows scrolling to the start of a `Prompt` zone; it takes an
argument that specifies the number of zones to move and the direction to move
in; `-1` means to move to the previous zone while `1` means to move to the next
zone.

This can make it convenient to skip over large amounts of output.

This action is not bound by default.

For the purposes of scrolling, the "current zone" is considered to be the one
closest to the top of the viewport.

```lua
local act = wezterm.action

config.keys = {
  { key = 'UpArrow', mods = 'SHIFT', action = act.ScrollToPrompt(-1) },
  { key = 'DownArrow', mods = 'SHIFT', action = act.ScrollToPrompt(1) },
}
```


