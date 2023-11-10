use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use prqlc_ast::{Span, Ty};

pub use prqlc_ast::stmt::QueryDef;

use super::expr::Expr;

// The following code is tested by the tests_misc crate to match stmt.rs in prqlc_ast.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stmt {
    #[serde(skip)]
    pub id: Option<usize>,
    #[serde(flatten)]
    pub kind: StmtKind,
    #[serde(skip)]
    pub span: Option<Span>,

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
    pub value: Box<Expr>,
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
