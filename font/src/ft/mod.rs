use failure::Error;
pub mod ftwrap;
pub mod hbwrap;
use self::ftwrap::Library;

pub struct FTEngine {
    lib: Library,
}


impl ::FontEngine for FTEngine {
    fn new() -> Result<FTEngine, Error> {
        Ok(FTEngine {
            lib: Library::new()?,
        })
    }
}
