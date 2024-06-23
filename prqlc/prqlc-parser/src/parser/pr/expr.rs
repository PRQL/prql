use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use crate::generic;
use crate::lexer::lr::Literal;
use crate::parser::pr::ops::{BinOp, UnOp};
use crate::parser::pr::Ty;
use crate::span::Span;

impl Expr {
    pub fn new<K: Into<ExprKind>>(kind: K) -> Self {
        Expr {
            kind: kind.into(),
            span: None,
            alias: None,
        }
    }
}

// The following code is tested by the tests_misc crate to match expr.rs in prqlc.

/// Expr is anything that has a value and thus a type.
/// Most of these can contain other [Expr] themselves; literals should be [ExprKind::Literal].
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
    Ident(String),
    Property {
        base: Box<Expr>,
        field: IndirectionKind,
    },
    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map")
    )]
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

impl ExprKind {
    pub fn into_expr(self, span: Span) -> Expr {
        Expr {
            span: Some(span),
            kind: self,
            alias: None,
        }
    }
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum IndirectionKind {
    Name(String),
    Position(i64),
    Star,
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
    pub return_ty: Option<Ty>,

    /// Expression containing parameter (and environment) references.
    pub body: Box<Expr>,

    /// Positional function parameters.
    pub params: Vec<FuncParam>,

    /// Named function parameters.
    pub named_params: Vec<FuncParam>,

    /// Generic type arguments within this function.
    pub generic_type_params: Vec<GenericTypeParam>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    pub default_value: Option<Box<Expr>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GenericTypeParam {
    /// Assigned name of this generic type argument.
    pub name: String,

    pub domain: Vec<Ty>,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub exprs: Vec<Expr>,
}

pub type Range = generic::Range<Box<Expr>>;
pub type InterpolateItem = generic::InterpolateItem<Expr>;
pub type SwitchCase = generic::SwitchCase<Box<Expr>>;

impl From<Literal> for ExprKind {
    fn from(value: Literal) -> Self {
        ExprKind::Literal(value)
    }
}

impl From<Func> for ExprKind {
    fn from(value: Func) -> Self {
        ExprKind::Func(Box::new(value))
    }
}
