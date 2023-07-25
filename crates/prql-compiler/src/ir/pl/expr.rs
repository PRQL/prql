use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use prql_ast::expr::{Ident, Literal};
use serde::{Deserialize, Serialize};

use prql_ast::expr::generic;
use prql_ast::Span;

pub use prql_ast::expr::{BinOp, UnOp};

use super::{Lineage, TransformCall, Ty, TyOrExpr};

// The following code is tested by the tests_misc crate to match expr.rs in prql_ast.

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

    /// Unique identificator of the node. Set exactly once during semantic::resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,

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

pub type Range = generic::Range<Box<Expr>>;
pub type InterpolateItem = generic::InterpolateItem<Expr>;
pub type SwitchCase = generic::SwitchCase<Box<Expr>>;

impl From<Vec<Expr>> for Pipeline {
    fn from(nodes: Vec<Expr>) -> Self {
        Pipeline { exprs: nodes }
    }
}
