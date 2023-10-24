use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::ir::generic::WindowKind;
use crate::ir::pl::{Expr, ExprKind, Func, FuncCall, Range, Ty};

impl FuncCall {
    pub fn new_simple(name: Expr, args: Vec<Expr>) -> Self {
        FuncCall {
            name: Box::new(name),
            args,
            named_args: Default::default(),
        }
    }
}

/// An expression that may have already been converted to a type.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum TyOrExpr {
    Ty(Ty),
    Expr(Box<Expr>),
}

impl Func {
    pub(crate) fn as_debug_name(&self) -> &str {
        let ident = self.name_hint.as_ref();

        ident.map(|n| n.name.as_str()).unwrap_or("<anonymous>")
    }
}

pub type WindowFrame = crate::ir::generic::WindowFrame<Box<Expr>>;
pub type ColumnSort = crate::ir::generic::ColumnSort<Box<Expr>>;

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

/// A reference to a table that is not in scope of this query.
///
/// > Note: We're not using this at the moment in
/// > [crate::ir::rq::RelationKind], since we wanted to avoid nested enums,
/// > since they can't be serialized to YAML at the moment. We may add this back
/// > in the future, or flatten it up to [crate::ir::rq::RelationKind]
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

impl Expr {
    pub fn new(kind: impl Into<ExprKind>) -> Self {
        Expr {
            id: None,
            kind: kind.into(),
            span: None,
            target_id: None,
            target_ids: Vec::new(),
            ty: None,
            lineage: None,
            needs_window: false,
            alias: None,
            flatten: false,
        }
    }
}
