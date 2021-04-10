### Font Related Configuration

WezTerm bundles [JetBrains Mono](https://www.jetbrains.com/lp/mono/),
[PowerlineExtraSymbols](https://github.com/ryanoasis/powerline-extra-symbols) and
[Noto Color Emoji](https://www.google.com/get/noto/help/emoji/) fonts
and uses those for the default font configuration.

If you wish to use a different font face, then you can use
the [wezterm.font](lua/wezterm/font.md) function to specify it:

```lua
local wezterm = require 'wezterm';

return {
  font = wezterm.font("Fira Code"),
  -- You can specify some parameters to influence the font selection;
  -- for example, this selects a Bold, Italic font variant.
  font = wezterm.font("JetBrains Mono", {weight="Bold", italic=true})
}
```

#### Fallback

WezTerm allows specifying an ordered list of fonts; when resolving
text into glyphs the first font in the list is consulted, and if the
glyph isn't present in that font, WezTerm proceeds to the next font
in the fallback list.

The default fallback includes the popular
[PowerlineExtraSymbols](https://github.com/ryanoasis/powerline-extra-symbols)
font, which means that you don't need to use specially patched fonts to use the
powerline glyphs.

You can specify your own fallback; that's useful if you've got a killer
monospace font, but it doesn't have glyphs for the asian script that you
sometimes work with:

```lua
local wezterm = require 'wezterm';
return {
  font = wezterm.font_with_fallback({
    "Fira Code",
    "DengXian",
  }),
}
```

WezTerm will still append its default fallback to whatever list you specify,
so you needn't worry about replicating that list if you set your own fallback.

If none of the fonts in the fallback list (including WezTerm's default fallback
list) contain a given glyph, then wezterm will resolve the system fallback list
and try those fonts too.  If a glyph cannot be resolved, wezterm will render a
special "Last Resort" glyph as a placeholder.  You may notice the placeholder
appear momentarily and then refresh itself to the system fallback glyph on some
systems.

### Font Related Options

Additional options for configuring fonts can be found elsewhere in the docs:

* [bold_brightens_ansi_colors](lua/config/bold_brightens_ansi_colors.md) - whether bold text uses the bright ansi palette
* [dpi](lua/config/dpi.md) - override the DPI; potentially useful for X11 users with high-density displays if experiencing tiny or blurry fonts
* [font_dirs](lua/config/font_dirs.md) - look for fonts in a set of directories
* [font_locator](lua/config/font_locator.md) - override the system font resolver
* [font_rules](lua/config/font_rules.md) - advanced control over which fonts are used for italic, bold and other textual styles
* [font_shaper](lua/config/font_shaper.md) - affects kerning and ligatures
* [font_size](lua/config/font_size.md) - change the size of the text
* [freetype_load_flags](lua/config/freetype_load_flags.md) - advanced hinting configuration
* [freetype_load_target](lua/config/freetype_load_target.md) - configure hinting and anti-aliasing
* [freetype_render_target](lua/config/freetype_render_target.md) - configure anti-aliasing
* [line_height](lua/config/line_height.md) - scale the font-specified line height
* [wezterm.font](lua/wezterm/font.md) - select a font based on family and style attributes
* [wezterm.font_with_fallback](lua/wezterm/font_with_fallback.md) - select a font from a list of candidates
