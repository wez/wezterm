use crate::bidi_class::BidiClass;
use crate::direction::Direction;
use crate::NO_LEVEL;

/// Maximum stack depth; UBA guarantees that it will never increase
/// in later versions of the spec.
pub const MAX_DEPTH: usize = 125;

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Level(pub i8);

impl Level {
    pub fn direction(self) -> Direction {
        Direction::with_level(self.0)
    }

    pub fn as_bidi_class(self) -> BidiClass {
        if self.0 % 2 == 1 {
            BidiClass::RightToLeft
        } else {
            BidiClass::LeftToRight
        }
    }

    pub fn removed_by_x9(self) -> bool {
        self.0 == NO_LEVEL
    }

    pub fn max(self, other: Level) -> Level {
        Level(self.0.max(other.0))
    }

    pub(crate) fn least_greater_even(self) -> Option<Level> {
        let level = if self.0 % 2 == 0 {
            self.0 + 2
        } else {
            self.0 + 1
        };
        if level as usize > MAX_DEPTH {
            None
        } else {
            Some(Self(level))
        }
    }

    pub(crate) fn least_greater_odd(self) -> Option<Level> {
        let level = if self.0 % 2 == 1 {
            self.0 + 2
        } else {
            self.0 + 1
        };
        if level as usize > MAX_DEPTH {
            None
        } else {
            Some(Self(level))
        }
    }
}
