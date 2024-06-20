use std::collections::HashMap;

use enum_as_inner::EnumAsInner;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::parser::pr::ident::Ident;
use crate::parser::pr::{Expr, Ty};
use crate::parser::WithAesthetics;
use crate::span::Span;
use crate::TokenKind;

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

    // Maybe should be Token?
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aesthetics_before: Vec<TokenKind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aesthetics_after: Vec<TokenKind>,
}

impl WithAesthetics for Stmt {
    fn with_aesthetics(
        self,
        aesthetics_before: Vec<TokenKind>,
        aesthetics_after: Vec<TokenKind>,
    ) -> Self {
        Stmt {
            aesthetics_before,
            aesthetics_after,
            ..self
        }
    }
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum StmtKind {
    QueryDef(Box<QueryDef>),
    VarDef(VarDef),
    TypeDef(TypeDef),
    ModuleDef(ModuleDef),
    ImportDef(ImportDef),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VarDef {
    pub kind: VarDefKind,
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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ImportDef {
    pub alias: Option<String>,
    pub name: Ident,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub expr: Box<Expr>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aesthetics_before: Vec<TokenKind>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub aesthetics_after: Vec<TokenKind>,
}

impl WithAesthetics for Annotation {
    fn with_aesthetics(
        self,
        aesthetics_before: Vec<TokenKind>,
        aesthetics_after: Vec<TokenKind>,
    ) -> Self {
        Annotation {
            aesthetics_before,
            aesthetics_after,
            ..self
        }
    }
}

impl Stmt {
    pub fn new(kind: StmtKind) -> Stmt {
        Stmt {
            kind,
            span: None,
            annotations: Vec::new(),
            aesthetics_before: Vec::new(),
            aesthetics_after: Vec::new(),
        }
    }
}
