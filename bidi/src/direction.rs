use crate::bidi_class::BidiClass;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
}

impl Direction {
    pub fn with_level(level: i8) -> Self {
        if level % 2 == 1 {
            Self::RightToLeft
        } else {
            Self::LeftToRight
        }
    }

    pub fn opposite(self) -> Self {
        if self == Direction::LeftToRight {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        }
    }

    pub fn as_bidi_class(self) -> BidiClass {
        match self {
            Self::RightToLeft => BidiClass::RightToLeft,
            Self::LeftToRight => BidiClass::LeftToRight,
        }
    }
}
