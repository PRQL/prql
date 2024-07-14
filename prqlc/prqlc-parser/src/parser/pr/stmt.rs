use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::parser::pr::ident::Ident;
use crate::parser::pr::{Expr, Ty};
use crate::parser::SupportsDocComment;
use crate::span::Span;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct QueryDef {
    #[schemars(with = "String")]
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum VarDefKind {
    Let,
    Into,
    Main,
}

// The following code is tested by the tests_misc crate to match stmt.rs in prql_compiler.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Stmt {
    #[serde(flatten)]
    pub kind: StmtKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub annotations: Vec<Annotation>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_comment: Option<String>,
}

impl SupportsDocComment for Stmt {
    fn with_doc_comment(self, doc_comment: Option<String>) -> Self {
        Stmt {
            doc_comment,
            ..self
        }
    }
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub enum StmtKind {
    QueryDef(Box<QueryDef>),
    VarDef(VarDef),
    TypeDef(TypeDef),
    ModuleDef(ModuleDef),
    ImportDef(ImportDef),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VarDef {
    pub kind: VarDefKind,
    pub name: String,
    pub value: Option<Box<Expr>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TypeDef {
    pub name: String,
    pub value: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModuleDef {
    pub name: String,
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImportDef {
    pub alias: Option<String>,
    pub name: Ident,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Annotation {
    pub expr: Box<Expr>,
}

impl Stmt {
    pub fn new(kind: StmtKind) -> Stmt {
        Stmt {
            kind,
            span: None,
            annotations: Vec::new(),
            doc_comment: None,
        }
    }
}
