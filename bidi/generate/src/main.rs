use anyhow::Context;
use std::io::Write;

fn parse_codepoint(s: &str) -> anyhow::Result<u32> {
    u32::from_str_radix(s.trim(), 16).with_context(|| s.to_string())
}

fn gen_class() -> anyhow::Result<()> {
    let data = std::fs::read_to_string("bidi/data/DerivedBidiClass.txt")
        .context("bidi/data/DerivedBidiClass.txt")?;

    struct Entry {
        start: u32,
        end: u32,
        bidi_class: String,
        comment: String,
    }

    impl Entry {
        fn parse(line: &str) -> anyhow::Result<Option<Self>> {
            let line = line.trim();
            if line.starts_with("#") || line.is_empty() {
                return Ok(None);
            }
            let fields: Vec<&str> = line.split(';').collect();

            let range_fields: Vec<&str> = fields[0].trim().split("..").collect();
            let start: u32 = parse_codepoint(range_fields[0])?;
            let end = if let Some(end) = range_fields.get(1) {
                parse_codepoint(end)?
            } else {
                start
            };

            let fields: Vec<&str> = fields[1].split('#').collect();
            let bidi_class = fields[0].trim().to_string();
            let comment = fields[1].trim().to_string();

            Ok(Some(Entry {
                start,
                end,
                bidi_class,
                comment,
            }))
        }
    }

    let mut entries = vec![];

    for line in data.lines() {
        if let Some(entry) = Entry::parse(line)? {
            entries.push(entry);
        }
    }

    entries.sort_by_key(|e| e.start);

    let mut f =
        std::fs::File::create("bidi/src/bidi_class.rs").context("bidi/src/bidi_class.rs")?;
    writeln!(
        f,
        "//! Generated from bidi/data/DerivedBidiClass.txt by bidi/generate/src/main.rs"
    )?;
    writeln!(
        f,
        r"
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum BidiClass {{
    ArabicLetter,
    ArabicNumber,
    BoundaryNeutral,
    CommonSeparator,
    EuropeanNumber,
    EuropeanSeparator,
    EuropeanTerminator,
    FirstStrongIsolate,
    LeftToRight,
    LeftToRightEmbedding,
    LeftToRightIsolate,
    LeftToRightOverride,
    NonspacingMark,
    OtherNeutral,
    ParagraphSeparator,
    PopDirectionalFormat,
    PopDirectionalIsolate,
    RightToLeft,
    RightToLeftEmbedding,
    RightToLeftIsolate,
    RightToLeftOverride,
    SegmentSeparator,
    WhiteSpace,
}}
"
    )?;
    writeln!(
        f,
        "pub const BIDI_CLASS: &'static [(char, char, BidiClass)] = &["
    )?;
    for entry in entries.into_iter() {
        writeln!(
            f,
            "  ('{}', '{}', {}), // {}",
            char::from_u32(entry.start).unwrap().escape_unicode(),
            char::from_u32(entry.end).unwrap().escape_unicode(),
            match entry.bidi_class.as_str() {
                "AL" => "BidiClass::ArabicLetter",
                "AN" => "BidiClass::ArabicNumber",
                "BN" => "BidiClass::BoundaryNeutral",
                "CS" => "BidiClass::CommonSeparator",
                "EN" => "BidiClass::EuropeanNumber",
                "ES" => "BidiClass::EuropeanSeparator",
                "ET" => "BidiClass::EuropeanTerminator",
                "FSI" => "BidiClass::FirstStrongIsolate",
                "L" => "BidiClass::LeftToRight",
                "LRO" => "BidiClass::LeftToRightOverride",
                "LRE" => "BidiClass::LeftToRightEmbedding",
                "LRI" => "BidiClass::LeftToRightIsolate",
                "NSM" => "BidiClass::NonspacingMark",
                "ON" => "BidiClass::OtherNeutral",
                "B" => "BidiClass::ParagraphSeparator",
                "PDF" => "BidiClass::PopDirectionalFormat",
                "PDI" => "BidiClass::PopDirectionalIsolate",
                "R" => "BidiClass::RightToLeft",
                "RLE" => "BidiClass::RightToLeftEmbedding",
                "RLI" => "BidiClass::RightToLeftIsolate",
                "RLO" => "BidiClass::RightToLeftOverride",
                "S" => "BidiClass::SegmentSeparator",
                "WS" => "BidiClass::WhiteSpace",
                bad => panic!("invalid BidiClass {}", bad),
            },
            entry.comment
        )?;
    }

    writeln!(f, "];")?;

    Ok(())
}

fn gen_brackets() -> anyhow::Result<()> {
    let data = std::fs::read_to_string("bidi/data/BidiBrackets.txt")
        .context("bidi/data/BidiBrackets.txt")?;

    struct Entry {
        code_point: u32,
        bidi_paired_bracket: u32,
        bidi_paired_bracket_type: char,
        comment: String,
    }

    impl Entry {
        fn parse(line: &str) -> anyhow::Result<Option<Self>> {
            let line = line.trim();
            if line.starts_with("#") || line.is_empty() {
                return Ok(None);
            }
            let fields: Vec<&str> = line.split(';').collect();

            let code_point: u32 = parse_codepoint(fields[0])?;
            let bidi_paired_bracket: u32 = parse_codepoint(fields[1])?;

            let fields: Vec<&str> = fields[2].split('#').collect();
            let bidi_paired_bracket_type: char = fields[0]
                .trim()
                .parse()
                .with_context(|| fields[0].to_string())?;
            let comment = fields[1].trim().to_string();

            Ok(Some(Entry {
                code_point,
                bidi_paired_bracket,
                bidi_paired_bracket_type,
                comment,
            }))
        }
    }

    let mut entries = vec![];

    for line in data.lines() {
        if let Some(entry) = Entry::parse(line)? {
            entries.push(entry);
        }
    }

    entries.sort_by_key(|e| e.code_point);

    let mut f =
        std::fs::File::create("bidi/src/bidi_brackets.rs").context("bidi/src/bidi_brackets.rs")?;
    writeln!(
        f,
        "//! Generated from bidi/data/BidiBrackets.txt by bidi/generate/src/main.rs"
    )?;
    writeln!(
        f,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq)] #[repr(u8)] pub enum BracketType {{ Open, Close }}"
    )?;
    writeln!(
        f,
        "pub const BIDI_BRACKETS: &'static [(char, char, BracketType)] = &["
    )?;
    for entry in entries.into_iter() {
        writeln!(
            f,
            "  ('{}', '{}', {}), // {}",
            char::from_u32(entry.code_point).unwrap().escape_unicode(),
            char::from_u32(entry.bidi_paired_bracket)
                .unwrap()
                .escape_unicode(),
            match entry.bidi_paired_bracket_type {
                'o' => "BracketType::Open",
                'c' => "BracketType::Close",
                bad => panic!("invalid BracketType {}", bad),
            },
            entry.comment
        )?;
    }

    writeln!(f, "];")?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    gen_brackets().context("gen_brackets")?;
    gen_class().context("gen_class")?;
    Ok(())
}
