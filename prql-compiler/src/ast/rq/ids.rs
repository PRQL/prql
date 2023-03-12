use serde::{Deserialize, Serialize};

/// Column id
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub struct CId(usize);

impl CId {
    pub fn get(&self) -> usize {
        self.0
    }
}

impl From<usize> for CId {
    fn from(id: usize) -> Self {
        CId(id)
    }
}

impl std::fmt::Debug for CId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "column-{}", self.0)
    }
}

/// Table id
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TId(usize);

impl TId {
    pub fn get(&self) -> usize {
        self.0
    }
}

impl From<usize> for TId {
    fn from(id: usize) -> Self {
        TId(id)
    }
}

impl std::fmt::Debug for TId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "table-{}", self.0)
    }
}
