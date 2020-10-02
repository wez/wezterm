use super::*;

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum FrontEndSelection {
    OpenGL,
    Software,
    OldSoftware,
    MuxServer,
    Null,
}
impl_lua_conversion!(FrontEndSelection);

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}
