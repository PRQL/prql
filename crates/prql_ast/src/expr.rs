pub mod generic;
mod ident;
mod literal;
mod ops;

use std::collections::HashMap;

pub use ident::Ident;
pub use literal::{Literal, ValueAndUnit};
pub use ops::{BinOp, UnOp};

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::Span;

impl Expr {
    pub fn new(kind: ExprKind) -> Self {
        Expr {
            kind,
            span: None,
            alias: None,
        }
    }
}

// The following code is tested by the tests_misc crate to match expr.rs in prql_compiler.

/// Expr is anything that has a value and thus a type.
/// If it cannot contain nested Exprs, is should be under [ExprKind::Literal].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr {
    #[serde(flatten)]
    pub kind: ExprKind,

    #[serde(skip)]
    pub span: Option<Span>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr)]
pub enum ExprKind {
    Ident(Ident),
    Literal(Literal),
    Pipeline(Pipeline),

    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Range(Range),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    FuncCall(FuncCall),
    Func(Box<Func>),
    SString(Vec<InterpolateItem>),
    FString(Vec<InterpolateItem>),
    Case(Vec<SwitchCase>),

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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UnaryExpr {
    pub op: UnOp,
    pub expr: Box<Expr>,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncCall {
    pub name: Box<Expr>,
    pub args: Vec<Expr>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub named_args: HashMap<String, Expr>,
}

/// Function called with possibly missing positional arguments.
/// May also contain environment that is needed to evaluate the body.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Func {
    /// Type requirement for the function body expression.
    pub return_ty: Option<Box<Expr>>,

    /// Expression containing parameter (and environment) references.
    pub body: Box<Expr>,

    /// Positional function parameters.
    pub params: Vec<FuncParam>,

    /// Named function parameters.
    pub named_params: Vec<FuncParam>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Box<Expr>>,

    pub default_value: Option<Box<Expr>>,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub exprs: Vec<Expr>,
}

pub type Range = generic::Range<Box<Expr>>;
pub type InterpolateItem = generic::InterpolateItem<Expr>;
pub type SwitchCase = generic::SwitchCase<Box<Expr>>;

impl From<Vec<Expr>> for Pipeline {
    fn from(nodes: Vec<Expr>) -> Self {
        Pipeline { exprs: nodes }
    }
}
