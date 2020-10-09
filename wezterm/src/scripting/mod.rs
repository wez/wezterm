pub mod guiwin;
pub mod pane;

fn luaerr(err: anyhow::Error) -> mlua::Error {
    mlua::Error::external(err)
}
