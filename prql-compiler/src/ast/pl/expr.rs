use std::collections::HashMap;

use anyhow::Result;
use enum_as_inner::EnumAsInner;

use prql_ast::Ident;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Reason, WithErrorInfo};
use prql_ast::expr::Extension;

use super::{Lineage, Ty};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct X;

impl Extension for X {
    type Span = crate::Span;

    type ExprExtra = ExprExtra;

    type ExprKindVariant = ExprKindExtra;

    type FuncExtra = FuncExtra;

    type FuncParamExtra = FuncParamExtra;
}

pub type Expr = prql_ast::expr::Expr<X>;
pub type ExprKind = prql_ast::expr::ExprKind<X>;
pub type Func = prql_ast::expr::Func<X>;
pub type FuncCall = prql_ast::expr::FuncCall<X>;
pub type FuncParam = prql_ast::expr::FuncParam<X>;
pub type Pipeline = prql_ast::expr::Pipeline<X>;
pub type UnaryExpr = prql_ast::expr::UnaryExpr<X>;
pub type BinaryExpr = prql_ast::expr::BinaryExpr<X>;

pub type Range = prql_ast::expr::Range<Box<Expr>>;
pub type InterpolateItem = prql_ast::expr::InterpolateItem<Expr>;
pub type SwitchCase = prql_ast::expr::SwitchCase<Expr>;

/// Expr is anything that has a value and thus a type.
/// If it cannot contain nested Exprs, is should be under [ExprKind::Literal].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExprExtra {
    /// Unique identificator of the node. Set exactly once during semantic::resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// For [Ident]s, this is id of node referenced by the ident
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<usize>,

    /// For [ExprKind::All], these are ids of included nodes
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub target_ids: Vec<usize>,

    /// Type of expression this node represents.
    /// [None] means that type should be inferred.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    /// Information about where data of this expression will come from.
    ///
    /// Currently, this is used to infer relational pipeline frames.
    /// Must always exists if ty is a relation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lineage: Option<Lineage>,

    #[serde(skip)]
    pub needs_window: bool,

    /// When true on [ExprKind::Tuple], this list will be flattened when placed
    /// in some other list.
    // TODO: maybe we should have a special ExprKind instead of this flag?
    #[serde(skip)]
    pub flatten: bool,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum ExprKindExtra {
    All { within: Ident, except: Vec<Expr> },
    TransformCall(TransformCall),

    RqOperator { name: String, args: Vec<Expr> },

    Type(Ty),
}

impl From<ExprKindExtra> for ExprKind {
    fn from(value: ExprKindExtra) -> Self {
        Self::Other(value)
    }
}

/// Function called with possibly missing positional arguments.
/// May also contain environment that is needed to evaluate the body.
#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub struct FuncExtra {
    /// Name of the function. Used for user-facing messages only.
    pub name_hint: Option<Ident>,

    /// Type requirement for the function body expression.
    pub return_ty: Option<Ty>,

    /// Arguments that have already been provided.
    pub args: Vec<Expr>,

    /// Additional variables that the body of the function may need to be
    /// evaluated.
    pub env: HashMap<String, Expr>,
}

#[derive(Debug, PartialEq, Clone, Default, Serialize, Deserialize)]
pub struct FuncParamExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct WindowFrame<T = Box<Expr>> {
    pub kind: WindowKind,
    pub range: prql_ast::expr::Range<T>,
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
            range: prql_ast::expr::Range::<T>::unbounded(),
        }
    }
}
