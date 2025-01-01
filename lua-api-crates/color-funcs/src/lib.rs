use crate::schemes::base16::Base16Scheme;
use crate::schemes::sexy::Sexy;
use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods, UserDataRef};
use config::lua::{get_or_create_module, get_or_create_sub_module};
use config::{ColorSchemeFile, ColorSchemeMetaData, Gradient, Palette, RgbaColor, SrgbaTuple};

mod image_colors;
pub mod schemes;

#[derive(Clone)]
pub struct ColorWrap(RgbaColor);

impl ColorWrap {
    pub fn complement(&self) -> Self {
        Self(self.0.complement().into())
    }
    pub fn complement_ryb(&self) -> Self {
        Self(self.0.complement_ryb().into())
    }
    pub fn triad(&self) -> (Self, Self) {
        let (a, b) = self.0.triad();
        (Self(a.into()), Self(b.into()))
    }
    pub fn square(&self) -> (Self, Self, Self) {
        let (a, b, c) = self.0.square();
        (Self(a.into()), Self(b.into()), Self(c.into()))
    }
    pub fn saturate(&self, factor: f64) -> Self {
        Self(self.0.saturate(factor).into())
    }
    pub fn saturate_fixed(&self, amount: f64) -> Self {
        Self(self.0.saturate_fixed(amount).into())
    }
    pub fn lighten(&self, factor: f64) -> Self {
        Self(self.0.lighten(factor).into())
    }
    pub fn lighten_fixed(&self, amount: f64) -> Self {
        Self(self.0.lighten_fixed(amount).into())
    }
    pub fn adjust_hue_fixed(&self, amount: f64) -> Self {
        Self(self.0.adjust_hue_fixed(amount).into())
    }
    pub fn adjust_hue_fixed_ryb(&self, amount: f64) -> Self {
        Self(self.0.adjust_hue_fixed_ryb(amount).into())
    }
}

impl UserData for ColorWrap {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            let s: String = this.0.into();
            Ok(s)
        });
        methods.add_meta_method(MetaMethod::Eq, |_, this, other: UserDataRef<ColorWrap>| {
            Ok(this.0 == other.0)
        });
        methods.add_method("complement", |_, this, _: ()| Ok(this.complement()));
        methods.add_method("complement_ryb", |_, this, _: ()| Ok(this.complement_ryb()));
        methods.add_method("triad", |_, this, _: ()| Ok(this.triad()));
        methods.add_method("square", |_, this, _: ()| Ok(this.square()));
        methods.add_method("saturate", |_, this, factor: f64| Ok(this.saturate(factor)));

        methods.add_method("desaturate", |_, this, factor: f64| {
            Ok(this.saturate(-factor))
        });

        methods.add_method("saturate_fixed", |_, this, amount: f64| {
            Ok(this.saturate_fixed(amount))
        });
        methods.add_method("desaturate_fixed", |_, this, amount: f64| {
            Ok(this.saturate_fixed(-amount))
        });

        methods.add_method("lighten", |_, this, factor: f64| Ok(this.lighten(factor)));

        methods.add_method("darken", |_, this, factor: f64| Ok(this.lighten(-factor)));

        methods.add_method("lighten_fixed", |_, this, amount: f64| {
            Ok(this.lighten_fixed(amount))
        });
        methods.add_method("darken_fixed", |_, this, amount: f64| {
            Ok(this.lighten_fixed(-amount))
        });

        methods.add_method("adjust_hue_fixed", |_, this, amount: f64| {
            Ok(this.adjust_hue_fixed(amount))
        });
        methods.add_method("adjust_hue_fixed_ryb", |_, this, amount: f64| {
            Ok(this.adjust_hue_fixed_ryb(amount))
        });
        methods.add_method("srgba_u8", |_, this, _: ()| Ok(this.0.to_srgb_u8()));
        methods.add_method("linear_rgba", |_, this, _: ()| {
            let rgba = this.0.to_linear();
            Ok((rgba.0, rgba.1, rgba.2, rgba.3))
        });
        methods.add_method("hsla", |_, this, _: ()| Ok(this.0.to_hsla()));
        methods.add_method("laba", |_, this, _: ()| Ok(this.0.to_laba()));
        methods.add_method(
            "contrast_ratio",
            |_, this, other: UserDataRef<ColorWrap>| Ok(this.0.contrast_ratio(&other.0)),
        );
        methods.add_method("delta_e", |_, this, other: UserDataRef<ColorWrap>| {
            Ok(this.0.delta_e(&other.0))
        });
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let color = get_or_create_sub_module(lua, "color")?;
    color.set("parse", lua.create_function(parse_color)?)?;
    color.set(
        "from_hsla",
        lua.create_function(|_, (h, s, l, a): (f64, f64, f64, f64)| {
            Ok(ColorWrap(SrgbaTuple::from_hsla(h, s, l, a).into()))
        })?,
    )?;
    color.set(
        "extract_colors_from_image",
        lua.create_function(image_colors::extract_colors_from_image)?,
    )?;
    color.set(
        "get_default_colors",
        lua.create_function(|_, _: ()| {
            let palette: Palette = wezterm_term::color::ColorPalette::default().into();
            Ok(palette)
        })?,
    )?;

    color.set(
        "load_scheme",
        lua.create_function(|_, file_name: String| {
            let data = std::fs::read_to_string(file_name)
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?;
            let scheme = ColorSchemeFile::from_toml_str(&data)
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?;
            Ok((scheme.colors, scheme.metadata))
        })?,
    )?;

    color.set(
        "save_scheme",
        lua.create_function(
            |_, (colors, metadata, file_name): (Palette, ColorSchemeMetaData, String)| {
                let scheme = ColorSchemeFile { colors, metadata };
                scheme
                    .save_to_file(file_name)
                    .map_err(|err| mlua::Error::external(format!("{err:#}")))
            },
        )?,
    )?;

    color.set(
        "load_terminal_sexy_scheme",
        lua.create_function(|_, file_name: String| {
            let scheme = Sexy::load_file(file_name)
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?;
            Ok((scheme.colors, scheme.metadata))
        })?,
    )?;
    color.set(
        "load_base16_scheme",
        lua.create_function(|_, file_name: String| {
            let scheme = Base16Scheme::load_file(file_name)
                .map_err(|err| mlua::Error::external(format!("{err:#}")))?;
            Ok((scheme.colors, scheme.metadata))
        })?,
    )?;

    let wezterm_mod = get_or_create_module(lua, "wezterm")?;
    wezterm_mod.set("gradient_colors", lua.create_function(gradient_colors)?)?;
    color.set("gradient", lua.create_function(gradient_colors)?)?;

    wezterm_mod.set(
        "get_builtin_color_schemes",
        lua.create_function(|_, ()| Ok(config::COLOR_SCHEMES.clone()))?,
    )?;
    color.set(
        "get_builtin_schemes",
        lua.create_function(|_, ()| Ok(config::COLOR_SCHEMES.clone()))?,
    )?;

    Ok(())
}

fn parse_color<'lua>(_: &'lua Lua, spec: String) -> mlua::Result<ColorWrap> {
    let color =
        RgbaColor::try_from(spec).map_err(|err| mlua::Error::external(format!("{err:#}")))?;
    Ok(ColorWrap(color))
}

fn gradient_colors<'lua>(
    _lua: &'lua Lua,
    (gradient, num_colors): (Gradient, usize),
) -> mlua::Result<Vec<ColorWrap>> {
    let g = gradient.build().map_err(mlua::Error::external)?;
    Ok(g.colors(num_colors)
        .into_iter()
        .map(|c| {
            let tuple = SrgbaTuple::from(c);
            ColorWrap(tuple.into())
        })
        .collect())
}
