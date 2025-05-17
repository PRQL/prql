use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::lexer::lr::Literal;
use crate::parser::pr::ops::{BinOp, UnOp};
use crate::parser::pr::{Ident, Ty};
use crate::span::Span;
use crate::{generic, parser::SupportsDocComment};

impl Expr {
    pub fn new<K: Into<ExprKind>>(kind: K) -> Self {
        Expr {
            kind: kind.into(),
            span: None,
            alias: None,
            doc_comment: None,
        }
    }
}

// The following code is tested by the tests_misc crate to match expr.rs in prqlc.

/// Expr is anything that has a value and thus a type.
/// Most of these can contain other [Expr] themselves; literals should be [ExprKind::Literal].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Expr {
    #[serde(flatten)]
    pub kind: ExprKind,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
}

impl SupportsDocComment for Expr {
    fn with_doc_comment(self, doc_comment: Option<String>) -> Self {
        Self {
            doc_comment,
            ..self
        }
    }
}

#[derive(
    Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, strum::AsRefStr, JsonSchema,
)]
pub enum ExprKind {
    Ident(Ident),

    #[cfg_attr(
        feature = "serde_yaml",
        serde(with = "serde_yaml::with::singleton_map"),
        schemars(with = "Literal")
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
            doc_comment: None,
        }
    }
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum IndirectionKind {
    Name(String),
    Position(i64),
    Star,
}

/// Expression with two operands and an operator, such as `1 + 2`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: BinOp,
    pub right: Box<Expr>,
}

/// Expression with one operand and an operator, such as `-1`.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnaryExpr {
    pub op: UnOp,
    pub expr: Box<Expr>,
}

/// Function call.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FuncCall {
    pub name: Box<Expr>,
    pub args: Vec<Expr>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub named_args: HashMap<String, Expr>,
}

/// Function called with possibly missing positional arguments.
/// May also contain environment that is needed to evaluate the body.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Func {
    /// Type requirement for the function body expression.
    pub return_ty: Option<Ty>,

    /// Expression containing parameter (and environment) references.
    pub body: Box<Expr>,

    /// Positional function parameters.
    pub params: Vec<FuncParam>,

    /// Named function parameters.
    pub named_params: Vec<FuncParam>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    pub default_value: Option<Box<Expr>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GenericTypeParam {
    /// Assigned name of this generic type argument.
    pub name: String,

    pub domain: Vec<Ty>,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
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
