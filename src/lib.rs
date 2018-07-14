#[macro_use]
extern crate failure;
extern crate palette;
extern crate serde;
extern crate terminfo;
#[macro_use]
extern crate serde_derive;
extern crate num;
extern crate vte;
#[macro_use]
extern crate num_derive;

pub mod cell;
pub mod color;
pub mod escape;
pub mod render;
pub mod screen;
