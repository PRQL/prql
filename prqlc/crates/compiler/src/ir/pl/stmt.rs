use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};

use prql_ast::Span;

pub use prql_ast::stmt::QueryDef;

use super::expr::Expr;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum VarDefKind {
    Let,
    Into,
    Main,
}

// The following code is tested by the tests_misc crate to match stmt.rs in prql_ast.

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
    pub name: Option<String>,
    pub value: Box<Expr>,
    pub ty_expr: Option<Box<Expr>>,
    pub kind: VarDefKind,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub name: String,
    pub value: Option<Box<Expr>>,
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
