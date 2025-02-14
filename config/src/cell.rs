use crate::Arc;
use crate::HashMap;
use wezterm_dynamic::{FromDynamic, ToDynamic};

#[derive(Clone, Debug, Eq, PartialEq, FromDynamic, ToDynamic)]
pub struct CellWidth {
    pub first: u32,
    pub last: u32,
    pub width: u8,
}

impl CellWidth {
    pub fn compile_to_map(cellwidths: Option<Vec<Self>>) -> Option<Arc<HashMap<u32,u8>>> {
        let cellwidths = cellwidths.as_ref()?;
        let mut map = HashMap::new();
        for cellwidth in cellwidths {
            for i in cellwidth.first..=cellwidth.last {
                map.insert(i, cellwidth.width);
            }
        }
        Some(map.into())
    }
}