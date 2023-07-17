use std::collections::HashMap;

use anyhow::Result;
use enum_as_inner::EnumAsInner;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Reason, Span, WithErrorInfo};
use ast::generic as generic_expr;

use super::*;

/// Expr is anything that has a value and thus a type.
/// If it cannot contain nested Exprs, is should be under [ExprKind::Literal].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    /// Unique identificator of the node. Set exactly once during semantic::resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[serde(flatten)]
    pub kind: ExprKind,
    #[serde(skip)]
    pub span: Option<Span>,

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// When true on [ExprKind::Tuple], this list will be flattened when placed
    /// in some other list.
    // TODO: maybe we should have a special ExprKind instead of this flag?
    #[serde(skip)]
    pub flatten: bool,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum ExprKind {
    Ident(Ident),
    All {
        within: Ident,
        except: Vec<Expr>,
    },
    Literal(Literal),
    Pipeline(Pipeline),

    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Range(Range),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    FuncCall(FuncCall),
    Func(Box<Func>),
    TransformCall(TransformCall),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Case(Vec<SwitchCase>),
    RqOperator {
        name: String,
        args: Vec<Expr>,
    },

    Type(Ty),

    /// placeholder for values provided after query is compiled
    Param(String),

    /// When used instead of function body, the function will be translated to a RQ operator.
    /// Contains ident of the RQ operator.
    Internal(String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: BinOp,
    pub right: Box<Expr>,
}

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
)]
pub enum BinOp {
    #[strum(to_string = "*")]
    Mul,
    #[strum(to_string = "//")]
    DivInt,
    #[strum(to_string = "/")]
    DivFloat,
    #[strum(to_string = "%")]
    Mod,
    #[strum(to_string = "+")]
    Add,
    #[strum(to_string = "-")]
    Sub,
    #[strum(to_string = "==")]
    Eq,
    #[strum(to_string = "!=")]
    Ne,
    #[strum(to_string = ">")]
    Gt,
    #[strum(to_string = "<")]
    Lt,
    #[strum(to_string = ">=")]
    Gte,
    #[strum(to_string = "<=")]
    Lte,
    #[strum(to_string = "~=")]
    RegexSearch,
    #[strum(to_string = "&&")]
    And,
    #[strum(to_string = "||")]
    Or,
    #[strum(to_string = "??")]
    Coalesce,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UnaryExpr {
    pub op: UnOp,
    pub expr: Box<Expr>,
}

#[derive(
    Debug,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Serialize,
    Deserialize,
    strum::Display,
    strum::EnumString,
)]
pub enum UnOp {
    #[strum(to_string = "-")]
    Neg,
    #[strum(to_string = "+")]
    Add, // TODO: rename to Pos
    #[strum(to_string = "!")]
    Not,
    #[strum(to_string = "==")]
    EqSelf,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Box<Expr>,
    pub args: Vec<Expr>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub named_args: HashMap<String, Expr>,
}

impl FuncCall {
    pub fn new_simple(name: Expr, args: Vec<Expr>) -> Self {
        FuncCall {
            name: Box::new(name),
            args,
            named_args: HashMap::new(),
        }
    }
}

/// An expression that may have already been converted to a type.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, EnumAsInner)]
pub enum TyOrExpr {
    Ty(Ty),
    Expr(Box<Expr>),
}

/// Function called with possibly missing positional arguments.
/// May also contain environment that is needed to evaluate the body.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Func {
    /// Name of the function. Used for user-facing messages only.
    pub name_hint: Option<Ident>,

    /// Type requirement for the function body expression.
    pub return_ty: Option<TyOrExpr>,

    /// Expression containing parameter (and environment) references.
    pub body: Box<Expr>,

    /// Positional function parameters.
    pub params: Vec<FuncParam>,

    /// Named function parameters.
    pub named_params: Vec<FuncParam>,

    /// Arguments that have already been provided.
    pub args: Vec<Expr>,

    /// Additional variables that the body of the function may need to be
    /// evaluated.
    pub env: HashMap<String, Expr>,
}

impl Func {
    pub(crate) fn as_debug_name(&self) -> &str {
        let ident = self.name_hint.as_ref();

        ident.map(|n| n.name.as_str()).unwrap_or("<anonymous>")
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<TyOrExpr>,

    pub default_value: Option<Box<Expr>>,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub exprs: Vec<Expr>,
}

pub type Range = generic_expr::Range<Box<Expr>>;
pub type InterpolateItem = generic_expr::InterpolateItem<Expr>;
pub type SwitchCase = generic_expr::SwitchCase<Box<Expr>>;

pub type WindowFrame = crate::generic::WindowFrame<Box<Expr>>;
pub type ColumnSort = crate::generic::ColumnSort<Box<Expr>>;

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

impl Expr {
    pub fn null() -> Expr {
        Expr::new(ExprKind::Literal(Literal::Null))
    }

    pub(crate) fn try_cast<T, F, S2: ToString>(
        self,
        f: F,
        who: Option<&str>,
        expected: S2,
    ) -> Result<T, Error>
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

impl Expr {
    pub fn new(kind: ExprKind) -> Self {
        Expr {
            id: None,
            kind,
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

impl From<Vec<Expr>> for Pipeline {
    fn from(nodes: Vec<Expr>) -> Self {
        Pipeline { exprs: nodes }
    }
}
