use clap::Parser;
use std::io::{Read, Result};
use termwiz::escape::parser::Parser as TWParser;
use termwiz::escape::{Action, ControlCode};

#[derive(Debug, Parser)]
/// This is a little utility that strips escape sequences from
/// stdin and prints the result on stdout.
/// It preserves only printable characters and CR, LF and HT.
///
/// This utility is part of WezTerm.
///
/// https://github.com/wez/wezterm
struct Opt {}

fn main() -> Result<()> {
    let _ = Opt::parse();
    let mut buf = [0u8; 4096];

    let mut parser = TWParser::new();

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
