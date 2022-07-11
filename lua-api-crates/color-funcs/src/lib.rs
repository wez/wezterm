use config::lua::get_or_create_sub_module;
use config::lua::mlua::{self, Lua, MetaMethod, UserData, UserDataMethods};
use config::RgbaColor;

#[derive(Clone)]
struct ColorWrap(RgbaColor);

impl ColorWrap {
    pub fn complement(&self) -> Self {
        Self(self.0.complement().into())
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
}

impl UserData for ColorWrap {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            let s: String = this.0.into();
            Ok(s)
        });
        methods.add_meta_method(MetaMethod::Eq, |_, this, other: ColorWrap| {
            Ok(this.0 == other.0)
        });
        methods.add_method("complement", |_, this, _: ()| Ok(this.complement()));
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
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let color = get_or_create_sub_module(lua, "color")?;
    color.set("parse", lua.create_function(parse_color)?)?;
    Ok(())
}

fn parse_color<'lua>(_: &'lua Lua, spec: String) -> mlua::Result<ColorWrap> {
    let color =
        RgbaColor::try_from(spec).map_err(|err| mlua::Error::external(format!("{err:#}")))?;
    Ok(ColorWrap(color))
}
