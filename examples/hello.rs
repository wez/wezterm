extern crate termwiz;
#[macro_use]
extern crate failure;

use failure::Error;

fn main() -> Result<(), Error> {
    bail!("woot");
}
