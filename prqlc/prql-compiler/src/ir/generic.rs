use serde::{Deserialize, Serialize};

use prqlc_ast::expr::generic::Range;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnSort<T> {
    pub direction: SortDirection,
    pub column: T,
}

#[derive(Debug, Clone, Serialize, Default, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct WindowFrame<T> {
    pub kind: WindowKind,
    pub range: prqlc_ast::expr::generic::Range<T>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
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
                range: Range {
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
            range: Range::unbounded(),
        }
    }
}
