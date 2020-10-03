use super::*;

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq, Eq)]
pub enum FrontEndSelection {
    OpenGL,
    Software,
    OldSoftware,
}
impl_lua_conversion!(FrontEndSelection);

impl Default for FrontEndSelection {
    fn default() -> Self {
        FrontEndSelection::OpenGL
    }
}

impl FrontEndSelection {
    // TODO: find or build a proc macro for this
    pub fn variants() -> Vec<&'static str> {
        vec!["OpenGL", "Software", "OldSoftware"]
    }
}

impl std::str::FromStr for FrontEndSelection {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_ref() {
            "software" => Ok(FrontEndSelection::Software),
            "oldsoftware" => Ok(FrontEndSelection::OldSoftware),
            "opengl" => Ok(FrontEndSelection::OpenGL),
            _ => Err(anyhow!(
                "{} is not a valid FrontEndSelection variant, possible values are {:?}",
                s,
                FrontEndSelection::variants()
            )),
        }
    }
}
