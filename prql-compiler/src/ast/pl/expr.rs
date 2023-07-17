use anyhow::Result;
use enum_as_inner::EnumAsInner;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Reason, WithErrorInfo};

pub use self::ast::*;

mod ast;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct WindowFrame<T = Box<Expr>> {
    pub kind: WindowKind,
    pub range: Range<T>,
}

/// FuncCall with better typing. Returns the modified table.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TransformCall {
    pub input: Box<Expr>,

    pub kind: Box<TransformKind>,

    /// Grouping of values in columns
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition: Vec<Expr>,

    /// Windowing frame of columns
    #[serde(default, skip_serializing_if = "WindowFrame::is_default")]
    pub frame: WindowFrame,

    /// Windowing order of columns
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<ColumnSort>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, EnumAsInner)]
pub enum TransformKind {
    Derive {
        assigns: Vec<Expr>,
    },
    Select {
        assigns: Vec<Expr>,
    },
    Filter {
        filter: Box<Expr>,
    },
    Aggregate {
        assigns: Vec<Expr>,
    },
    Sort {
        by: Vec<ColumnSort>,
    },
    Take {
        range: Range,
    },
    Join {
        side: JoinSide,
        with: Box<Expr>,
        filter: Box<Expr>,
    },
    Group {
        by: Vec<Expr>,
        pipeline: Box<Expr>,
    },
    Window {
        kind: WindowKind,
        range: Range,
        pipeline: Box<Expr>,
    },
    Append(Box<Expr>),
    Loop(Box<Expr>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnSort<T = Box<Expr>> {
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
pub enum WindowKind {
    Rows,
    Range,
}

/// A reference to a table that is not in scope of this query.
///
/// > Note: We're not using this at the moment in
/// > [crate::ast::rq::RelationKind], since we wanted to avoid nested enums,
/// > since they can't be serialized to YAML at the moment. We may add this back
/// > in the future, or flatten it up to [crate::ast::rq::RelationKind]
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum TableExternRef {
    /// Actual table in a database, that we can refer to by name in SQL
    LocalTable(String),

    /// Placeholder for a relation that will be provided later.
    /// This is very similar to relational s-strings and may not even be needed for now, so
    /// it's not documented anywhere. But it will be used in the future.
    Param(String),
    // TODO: add other sources such as files, URLs
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum JoinSide {
    Inner,
    Left,
    Right,
    Full,
}

pub(crate) trait TryCast {
    fn try_cast<T, F, S2: ToString>(
        self,
        f: F,
        who: Option<&str>,
        expected: S2,
    ) -> Result<T, Error>
    where
        F: FnOnce(ExprKind) -> Result<T, ExprKind>;
}

impl TryCast for Expr {
    fn try_cast<T, F, S2: ToString>(self, f: F, who: Option<&str>, expected: S2) -> Result<T, Error>
    where
        F: FnOnce(ExprKind) -> Result<T, ExprKind>,
    {
        f(self.kind).map_err(|i| {
            Error::new(Reason::Expected {
                who: who.map(|s| s.to_string()),
                expected: expected.to_string(),
                found: format!("`{}`", Expr::new(i)),
            })
            .with_span(self.span)
        })
    }
}

impl WindowFrame {
    fn is_default(&self) -> bool {
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
