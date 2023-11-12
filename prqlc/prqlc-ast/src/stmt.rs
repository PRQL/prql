use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::{expr::Expr, Span, Ty};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum VarDefKind {
    Let,
    Into,
    Main,
}

// The following code is tested by the tests_misc crate to match stmt.rs in prql_compiler.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stmt {
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
    pub kind: VarDefKind,
    pub name: String,
    pub value: Box<Expr>,

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

impl Stmt {
    pub fn new(kind: StmtKind) -> Stmt {
        Stmt {
            kind,
            span: None,
            annotations: Vec::new(),
        }
    }
}
