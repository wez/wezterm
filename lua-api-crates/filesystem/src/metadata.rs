use config::lua::mlua::{self, MetaMethod, UserData, UserDataMethods};

#[derive(Debug, Clone)]
pub struct MetaData(pub smol::fs::Metadata);

impl UserData for MetaData {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("{:#?}", this.0))
        });
        methods.add_method("is_dir", |_, this, _: ()| {
            let b = this.0.is_dir();
            Ok(b)
        });
        methods.add_method("is_file", |_, this, _: ()| {
            let b = this.0.is_file();
            Ok(b)
        });
        methods.add_method("is_symlink", |_, this, _: ()| {
            let b = this.0.is_symlink();
            Ok(b)
        });
        methods.add_method("is_readonly", |_, this, _: ()| {
            let b = this.0.permissions().readonly();
            Ok(b)
        });
        methods.add_method("secs_since_modified", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .modified()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("secs_since_accessed", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .accessed()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("secs_since_created", |_, this, _: ()| {
            let elapsed_in_secs = this
                .0
                .created()
                .map_err(mlua::Error::external)?
                .elapsed()
                .map_err(mlua::Error::external)?
                .as_secs();
            Ok(elapsed_in_secs as i64)
        });
        methods.add_method("bytes", |_, this, _: ()| {
            let bytes = this.0.len();
            Ok(bytes as i64)
        });
    }
}
