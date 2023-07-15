use std::collections::HashMap;
use std::fmt::Debug;

use enum_as_inner::EnumAsInner;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::{ident::Ident, literal::Literal};

pub trait Extension: 'static + Default + Clone + Serialize {
    type Span: Debug + Clone + PartialEq + Serialize + DeserializeOwned;
    type ExprExtra: Default + Debug + Clone + PartialEq + Serialize + DeserializeOwned;
    type ExprKindVariant: Debug + Clone + PartialEq + Serialize + DeserializeOwned;
    type FuncExtra: Default + Debug + Clone + PartialEq + Serialize + DeserializeOwned;
    type FuncParamExtra: Default + Debug + Clone + PartialEq + Serialize + DeserializeOwned;
}

/// Expr is anything that has a value and thus a type.
/// If it cannot contain nested Exprs, is should be under [ExprKind::Literal].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expr<T: Extension> {
    #[serde(flatten)]
    pub kind: ExprKind<T>,
    #[serde(skip)]
    pub span: Option<T::Span>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    #[serde(flatten)]
    pub extra: T::ExprExtra,
}

impl<T: Extension> Expr<T> {
    pub fn new(kind: impl Into<ExprKind<T>>) -> Self {
        Expr {
            kind: kind.into(),
            span: None,
            alias: None,
            extra: Default::default(),
        }
    }

    pub fn null() -> Self {
        Expr::new(ExprKind::Literal(Literal::Null))
    }
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Deserialize, strum::AsRefStr)]
pub enum ExprKind<T: Extension> {
    Ident(Ident),
    Literal(Literal),
    Pipeline(Pipeline<T>),

    Tuple(Vec<Expr<T>>),
    Array(Vec<Expr<T>>),
    Range(Range<Box<Expr<T>>>),
    Binary(BinaryExpr<T>),
    Unary(UnaryExpr<T>),
    FuncCall(FuncCall<T>),
    Func(Box<Func<T>>),
    SString(Vec<InterpolateItem<Expr<T>>>),
    FString(Vec<InterpolateItem<Expr<T>>>),
    Case(Vec<SwitchCase<Box<Expr<T>>>>),

    /// placeholder for values provided after query is compiled
    Param(String),

    /// When used instead of function body, the function will be translated to a RQ operator.
    /// Contains ident of the RQ operator.
    Internal(String),

    Other(T::ExprKindVariant),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct BinaryExpr<T: Extension> {
    pub left: Box<Expr<T>>,
    pub op: BinOp,
    pub right: Box<Expr<T>>,
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
pub struct UnaryExpr<T: Extension> {
    pub op: UnOp,
    pub expr: Box<Expr<T>>,
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
pub struct FuncCall<T: Extension> {
    pub name: Box<Expr<T>>,
    pub args: Vec<Expr<T>>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub named_args: HashMap<String, Expr<T>>,
}

impl<T: Extension> FuncCall<T> {
    pub fn new_simple(name: Expr<T>, args: Vec<Expr<T>>) -> Self {
        FuncCall {
            name: Box::new(name),
            args,
            named_args: HashMap::new(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Func<T: Extension> {
    pub return_ty_expr: Option<Expr<T>>,

    /// Expression containing parameter (and environment) references.
    pub body: Box<Expr<T>>,

    /// Positional function parameters.
    pub params: Vec<FuncParam<T>>,

    /// Named function parameters.
    pub named_params: Vec<FuncParam<T>>,

    #[serde(flatten)]
    pub extra: T::FuncExtra,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam<T: Extension> {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty_expr: Option<Expr<T>>,

    pub default_value: Option<Box<Expr<T>>>,

    #[serde(flatten)]
    pub extra: T::FuncParamExtra,
}

/// A value and a series of functions that are to be applied to that value one after another.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Pipeline<T: Extension> {
    pub exprs: Vec<Expr<T>>,
}

impl<T: Extension> From<Vec<Expr<T>>> for Pipeline<T> {
    fn from(nodes: Vec<Expr<T>>) -> Self {
        Pipeline { exprs: nodes }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum InterpolateItem<E> {
    String(String),
    Expr {
        expr: Box<E>,
        format: Option<String>,
    },
}

/// Inclusive-inclusive range.
/// Missing bound means unbounded range.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range<E> {
    pub start: Option<E>,
    pub end: Option<E>,
}

impl<T> Range<T> {
    pub const fn unbounded() -> Self {
        Range {
            start: None,
            end: None,
        }
    }

    pub fn try_map<U, E, F: Fn(T) -> Result<U, E>>(self, f: F) -> Result<Range<U>, E> {
        Ok(Range {
            start: self.start.map(&f).transpose()?,
            end: self.end.map(f).transpose()?,
        })
    }

    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> Range<U> {
        Range {
            start: self.start.map(&f),
            end: self.end.map(f),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SwitchCase<E> {
    pub condition: E,
    pub value: E,
}
