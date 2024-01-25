use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use prqlc_ast::{Span, Ty};

pub use prqlc_ast::stmt::QueryDef;

use super::expr::Expr;

// The following code is tested by the tests_misc crate to match stmt.rs in prqlc_ast.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stmt {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<usize>,
    #[serde(flatten)]
    pub kind: StmtKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum StmtKind {
    QueryDef(Box<QueryDef>),
    VarDef(VarDef),
    TypeDef(TypeDef),
    ModuleDef(ModuleDef),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VarDef {
    pub name: String,
    pub value: Option<Box<Expr>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String,
    pub value: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ModuleDef {
    pub name: String,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub expr: Box<Expr>,
}
