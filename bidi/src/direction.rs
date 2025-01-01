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

    /// Given a DoubleEndedIterator, returns an iterator that will
    /// either walk it in its natural order if Direction==LeftToRight,
    /// or in reverse order if Direction==RightToLeft
    pub fn iter<I: DoubleEndedIterator<Item = T>, T>(self, iter: I) -> DirectionIter<I, T> {
        DirectionIter::wrap(iter, self)
    }
}

pub enum DirectionIter<I: DoubleEndedIterator<Item = T>, T> {
    LTR(I),
    RTL(std::iter::Rev<I>),
}

impl<I: DoubleEndedIterator<Item = T>, T> DirectionIter<I, T> {
    pub fn wrap(iter: I, direction: Direction) -> Self {
        match direction {
            Direction::LeftToRight => Self::LTR(iter),
            Direction::RightToLeft => Self::RTL(iter.rev()),
        }
    }
}

impl<I: DoubleEndedIterator<Item = T>, T> Iterator for DirectionIter<I, T> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::LTR(i) => i.next(),
            Self::RTL(i) => i.next(),
        }
    }
}
