use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use prqlc_parser::generic;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
pub struct ColumnSort<T> {
    pub direction: SortDirection,
    pub column: T,
}

#[derive(Debug, Clone, Serialize, Default, Deserialize, PartialEq, Eq, JsonSchema)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WindowFrame<T> {
    pub kind: WindowKind,
    pub range: generic::Range<T>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum WindowKind {
    Rows,
    Range,
}

impl<T> WindowFrame<T> {
    pub(crate) fn is_default(&self) -> bool {
        matches!(
            self,
            WindowFrame {
                kind: WindowKind::Rows,
                range: generic::Range {
                    start: None,
                    end: None
                }
            }
        )
    }
}

impl<T> Default for WindowFrame<T> {
    fn default() -> Self {
        Self {
            kind: WindowKind::Rows,
            range: generic::Range::unbounded(),
        }
    }
}
