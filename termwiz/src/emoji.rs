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
            if c == '\u{FE0F}' {
                // Explicit emoji presentation
                return (presentation, Some(Self::Emoji));
            } else if c == '\u{FE0E}' {
                return (presentation, Some(Self::Text));
            } else if Self::for_char(c) == Self::Emoji {
                presentation = Self::Emoji;
            }
            // Note that `c` may be some other combining
            // sequence that doesn't definitively indicate
            // that we're text, so we only positively
            // change presentation when we identify an
            // emoji char.
        }
        (presentation, None)
    }

    pub fn for_char(c: char) -> Self {
        if crate::emoji_presentation::EMOJI_PRESENTATION.contains_u32(c as u32) {
            Self::Emoji
        } else {
            Self::Text
        }
    }
}
