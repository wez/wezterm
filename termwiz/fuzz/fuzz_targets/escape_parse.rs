#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut p = termwiz::escape::parser::Parser::new();
    p.parse(data, |_| {});
});
