use std::{collections::HashMap, fmt::Display};

use anyhow::anyhow;
use enum_as_inner::EnumAsInner;
use semver::VersionReq;
use serde::{Deserialize, Serialize};

use crate::error::Span;

use super::*;

/// A helper wrapper around Vec<Stmt> so we can impl Display.
pub struct Statements(pub Vec<Stmt>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stmt {
    #[serde(skip)]
    pub id: Option<usize>,
    #[serde(flatten)]
    pub kind: StmtKind,
    #[serde(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum StmtKind {
    QueryDef(QueryDef),
    FuncDef(FuncDef),
    VarDef(VarDef),
    Main(Box<Expr>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub other: HashMap<String, String>,
}

/// Function definition.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncDef {
    pub name: String,
    pub positional_params: Vec<FuncParam>, // ident
    pub named_params: Vec<FuncParam>,      // named expr
    pub body: Box<Expr>,
    pub return_ty: Option<Ty>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,

    pub default_value: Option<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VarDef {
    pub name: String,
    pub value: Box<Expr>,
}

impl From<StmtKind> for Stmt {
    fn from(kind: StmtKind) -> Self {
        Stmt {
            kind,
            span: None,
            id: None,
        }
    }
}

impl From<StmtKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(item: StmtKind) -> Self {
        anyhow!("Failed to convert statement `{item}`")
    }
}

impl Display for Statements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for stmt in &self.0 {
            write!(f, "{}", stmt.kind)?;
            write!(f, "\n\n")?;
        }
        Ok(())
    }
}

impl Display for StmtKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
            StmtKind::Main(expr) => match &expr.kind {
                ExprKind::Pipeline(pipeline) => {
                    for expr in &pipeline.exprs {
                        writeln!(f, "{expr}")?;
                    }
                }
                _ => writeln!(f, "{}", expr)?,
            },
            StmtKind::FuncDef(func_def) => {
                writeln!(f, "{func_def}\n")?;
            }
            StmtKind::VarDef(var) => {
                let pipeline = &var.value;
                match &pipeline.kind {
                    ExprKind::FuncCall(_) => {
                        write!(f, "let {} = (\n  {pipeline}\n)\n\n", var.name)?;
                    }

                    _ => {
                        write!(f, "let {} = {pipeline}\n\n", var.name)?;
                    }
                };
            }
        }
        Ok(())
    }
}

impl Display for FuncDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func {}", self.name)?;
        for arg in &self.positional_params {
            write!(f, " {}", arg.name)?;
        }
        for arg in &self.named_params {
            write!(f, " {}:{}", arg.name, arg.default_value.as_ref().unwrap())?;
        }
        write!(f, " -> {}", self.body)
    }
}
