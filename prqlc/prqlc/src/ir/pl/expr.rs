use std::collections::HashMap;

use enum_as_inner::EnumAsInner;

use serde::{Deserialize, Serialize};

use prqlc_ast::expr::generic;
use prqlc_ast::{Ident, Literal, Span, Ty};

use crate::codegen::write_ty;

use super::{Lineage, TransformCall};

// The following code is tested by the tests_misc crate to match expr.rs in prqlc_ast.

/// Expr is anything that has a value and thus a type.
/// Most of these can contain other [Expr] themselves; literals should be [ExprKind::Literal].
#[derive(Clone, PartialEq, Serialize, Deserialize)]
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
        within: Box<Expr>,
        except: Box<Expr>,
    },
    Literal(Literal),

    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
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

    /// placeholder for values provided after query is compiled
    Param(String),

    /// When used instead of function body, the function will be translated to a RQ operator.
    /// Contains ident of the RQ operator.
    Internal(String),
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
    pub return_ty: Option<Ty>,

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
    pub ty: Option<Ty>,

    pub default_value: Option<Box<Expr>>,
}

pub type Range = generic::Range<Box<Expr>>;
pub type InterpolateItem = generic::InterpolateItem<Expr>;
pub type SwitchCase = generic::SwitchCase<Box<Expr>>;

impl From<Literal> for ExprKind {
    fn from(value: Literal) -> Self {
        ExprKind::Literal(value)
    }
}

impl From<Ident> for ExprKind {
    fn from(value: Ident) -> Self {
        ExprKind::Ident(value)
    }
}

impl From<Func> for ExprKind {
    fn from(value: Func) -> Self {
        ExprKind::Func(Box::new(value))
    }
}

impl std::fmt::Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Expr");

        if let Some(x) = &self.span {
            ds.field("span", x);
        }
        ds.field("kind", &self.kind);
        if let Some(x) = &self.alias {
            ds.field("alias", x);
        }
        if let Some(x) = &self.id {
            ds.field("id", x);
        }
        if let Some(x) = &self.target_id {
            ds.field("target_id", x);
        }
        if self.needs_window {
            ds.field("needs_window", &self.needs_window);
        }
        if self.flatten {
            ds.field("flatten", &self.flatten);
        }
        if let Some(x) = &self.ty {
            // DebugTy is needed to get around string quotes that
            // would be printed if we Debug-ed the string directly.
            struct DebugTy<'a>(&'a Ty);
            impl std::fmt::Debug for DebugTy<'_> {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.write_str(&write_ty(self.0))
                }
            }
            ds.field("ty", &DebugTy(x));
        }
        if let Some(x) = &self.lineage {
            ds.field("lineage", x);
        }
        ds.finish()
    }
}
