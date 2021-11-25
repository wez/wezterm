use std::collections::HashMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Presentation {
    Text,
    Emoji,
}

impl Presentation {
    /// Returns the default presentation followed
    /// by the explicit presentation if specified
    /// by a variation selector
    pub fn for_grapheme(s: &str) -> (Self, Option<Self>) {
        let mut presentation = Self::Text;
        for c in s.chars() {
            if Self::for_char(c) == Self::Emoji {
                presentation = Self::Emoji;
                break;
            }
            // Note that `c` may be some other combining
            // sequence that doesn't definitively indicate
            // that we're text, so we only positively
            // change presentation when we identify an
            // emoji char.
        }
        (presentation, VARIATION_MAP.get(s).copied())
    }

    pub fn for_char(c: char) -> Self {
        if crate::emoji_presentation::EMOJI_PRESENTATION.contains_u32(c as u32) {
            Self::Emoji
        } else {
            Self::Text
        }
    }
}

const VARIATION_SEQUENCES: &str = include_str!("../data/emoji-variation-sequences.txt");
lazy_static::lazy_static! {
    static ref VARIATION_MAP: HashMap<String, Presentation> = build_variation_sequences();
}

/// Parses emoji-variation-sequences.txt, which is part of the UCD download
/// for a given version of the Unicode spec.
/// It defines which sequences can have explicit presentation selectors.
fn build_variation_sequences() -> HashMap<String, Presentation> {
    let mut res = HashMap::new();

    'next_line: for line in VARIATION_SEQUENCES.lines() {
        if let Some(lhs) = line.split('#').next() {
            if let Some(seq) = lhs.split(';').next() {
                let mut s = String::new();
                let mut last = None;
                for hex in seq.split_whitespace() {
                    match u32::from_str_radix(hex, 16) {
                        Ok(n) => {
                            let c = char::from_u32(n).unwrap();
                            s.push(c);
                            last.replace(c);
                        }
                        Err(_) => {
                            continue 'next_line;
                        }
                    }
                }

                if let Some(last) = last {
                    res.insert(
                        s,
                        match last {
                            '\u{FE0F}' => Presentation::Emoji,
                            '\u{FE0E}' => Presentation::Text,
                            _ => unreachable!(),
                        },
                    );
                }
            }
        }
    }

    res
}
