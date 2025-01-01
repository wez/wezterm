---
tags:
  - font
---
# `freetype_pcf_long_family_names = false`

{{since('20220624-141144-bd1b7c5d')}}

This option provides control over the
[no-long-family-names](https://freetype.org/freetype2/docs/reference/ft2-properties.html#no-long-family-names)
FreeType PCF font driver property.

The default is for this configuration to be `false` which sets the PCF
driver to use the un-decorated font name. This corresponds to the
default mode of operation of the freetype library.

Some Linux distributions build the freetype library in a way that
causes the PCF driver to report font names differently; instead of
reporting just `Terminus` it will prefix the font name with the
foundry (`xos4` in the case of `Terminus`) and potentially append
`Wide` to the name if the font has wide glyphs.  The purpose of that
configuration option is to disambiguate fonts, as there are a number
of fonts from different foundries that all have the name `Fixed`, and
being presented with multiple items with the same `Fixed` label is a
very ambiguous user experience.

When two different applications have differing values for this long
family names property, they will face inconsistencies in resolving
fonts by name as they will disagree on what the name of a given PCF
font is.

## When should you set this option to true?

If all of the following are true, then you should set this option to
true:

* You need to use PCF fonts and you need to use `fontconfig` to resolve their names to font files.
* You are using a Linux distribution that builds their FreeType library with `PCF_CONFIG_OPTION_LONG_FAMILY_NAMES` defined.

Note that PCF fonts are a legacy font format and you will be better
served by OTF, TTF or OTB (open type binary) file formats.

## Why doesn't wezterm use the distro FreeType or match its configuration?

For the sake of consistency, wezterm vendors in its own copy of
the latest version FreeType and builds that same version on all
platforms.  The result is that font-related behaviors in a given
version of wezterm are the same on all platforms regardless of
what (potentially old) version of FreeType may be provided by
the distribution.

Not only does this provide consistency at runtime, but it is much
simpler to reason about at build time, making it simpler to build
wezterm on all systems.
