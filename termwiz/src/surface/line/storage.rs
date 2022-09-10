use crate::surface::line::cellref::CellRef;
use crate::surface::line::clusterline::{ClusterLineCellIter, ClusteredLine};
use crate::surface::line::vecstorage::{VecStorage, VecStorageIter};
#[cfg(feature = "use_serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CellStorage {
    V(VecStorage),
    C(ClusteredLine),
}

pub(crate) enum VisibleCellIter<'a> {
    V(VecStorageIter<'a>),
    C(ClusterLineCellIter<'a>),
}

impl<'a> Iterator for VisibleCellIter<'a> {
    type Item = CellRef<'a>;

    fn next(&mut self) -> Option<CellRef<'a>> {
        match self {
            Self::V(iter) => iter.next(),
            Self::C(iter) => iter.next(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn memory_usage() {
        assert_eq!(std::mem::size_of::<CellStorage>(), 72);
        assert_eq!(std::mem::size_of::<VecStorage>(), 24);
    }
}
