use std::{collections::HashMap, fmt::Display};

use anyhow::{anyhow, bail};
use enum_as_inner::EnumAsInner;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use super::expr::{Expr, ExprKind, Extension};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stmt<T: Extension> {
    #[serde(skip)]
    pub id: Option<usize>,
    pub name: String,
    #[serde(flatten)]
    pub kind: StmtKind<T>,
    #[serde(skip)]
    pub span: Option<T::Span>,

    pub annotations: Vec<Annotation<T>>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum StmtKind<T: Extension> {
    QueryDef(Box<QueryDef>),
    VarDef(VarDef<T>),
    TypeDef(TypeDef<T>),
    ModuleDef(ModuleDef<T>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VarDef<T: Extension> {
    pub value: Box<Expr<T>>,
    pub ty_expr: Option<Box<Expr<T>>>,
    pub kind: VarDefKind,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum VarDefKind {
    Let,
    Into,
    Main,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TypeDef<T: Extension> {
    pub value: Option<Box<Expr<T>>>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ModuleDef<T: Extension> {
    pub stmts: Vec<Stmt<T>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation<T: Extension> {
    pub expr: Box<Expr<T>>,
}

impl<T: Extension> Annotation<T> {
    /// Find the items in a `@{a=b}`. We're only using annotations with tuples;
    /// we can consider formalizing this constraint.
    pub fn tuple_items(self) -> anyhow::Result<Vec<(String, ExprKind<T>)>> {
        match self.expr.kind {
            ExprKind::Tuple(items) => items
                .into_iter()
                .map(|item| Ok((item.alias.clone().unwrap(), item.kind)))
                .collect(),
            _ => bail!("Annotation must be a tuple"),
        }
    }
}

impl<T: Extension> From<StmtKind<T>> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(_: StmtKind<T>) -> Self {
        anyhow!("Failed to convert statement")
    }
}

impl<T: Extension> Display for Stmt<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            StmtKind::QueryDef(query) => {
                write!(f, "prql")?;
                if let Some(version) = &query.version {
                    write!(f, " version:{}", version)?;
                }
                for (key, value) in &query.other {
                    write!(f, " {key}:{value}")?;
                }
                write!(f, "\n\n")?;
            }
            StmtKind::VarDef(var) => {
                let pipeline = &var.value;
                match &pipeline.kind {
                    ExprKind::FuncCall(_) => {
                        write!(f, "let {} = (\n  {pipeline}\n)\n\n", self.name)?;
                    }

                    _ => {
                        write!(f, "let {} = {pipeline}\n\n", self.name)?;
                    }
                };
            }
            StmtKind::TypeDef(ty_def) => {
                if let Some(value) = &ty_def.value {
                    write!(f, "type {} = {value}\n\n", self.name)?;
                } else {
                    write!(f, "type {}\n\n", self.name)?;
                }
            }
            StmtKind::ModuleDef(module_def) => {
                write!(f, "module {} {{", self.name)?;
                for stmt in &module_def.stmts {
                    write!(f, "{}", stmt)?;
                }
                write!(f, "}}\n\n")?;
            }
        }
        Ok(())
    }
}
