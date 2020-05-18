//! This is a little utility that strips escape sequences from
//! stdin and prints the result on stdout.
//! It preserves only printable characters and CL, LF and HT.
use std::io::{Read, Result};
use termwiz::escape::parser::Parser;
use termwiz::escape::{Action, ControlCode};

fn main() -> Result<()> {
    let mut buf = [0u8; 4096];

    let mut parser = Parser::new();

    loop {
        let len = std::io::stdin().read(&mut buf)?;
        if len == 0 {
            return Ok(());
        }

        parser.parse(&buf[0..len], |action| match action {
            Action::Print(c) => print!("{}", c),
            Action::Control(c) => match c {
                ControlCode::HorizontalTab
                | ControlCode::LineFeed
                | ControlCode::CarriageReturn => print!("{}", c as u8 as char),
                _ => {}
            },
            _ => {}
        });
    }
}
