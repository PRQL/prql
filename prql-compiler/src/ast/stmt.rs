use std::fmt::Display;

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
    #[serde(flatten)]
    pub kind: StmtKind,
    #[serde(skip)]
    pub span: Option<Span>,
}

#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
pub enum StmtKind {
    QueryDef(QueryDef),
    FuncDef(FuncDef),
    TableDef(TableDef),
    Pipeline(Box<Expr>),
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default)]
pub struct QueryDef {
    pub version: Option<VersionReq>,
    #[serde(default)]
    pub dialect: Dialect,
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
pub struct TableDef {
    pub name: String,
    pub value: Box<Expr>,
    pub id: Option<usize>,
}

impl From<StmtKind> for Stmt {
    fn from(kind: StmtKind) -> Self {
        Stmt { kind, span: None }
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
                write!(f, "prql dialect:{}", query.dialect)?;
                if let Some(version) = &query.version {
                    write!(f, " version:{}", version)?
                };
                write!(f, "\n\n")?;
            }
            StmtKind::Pipeline(expr) => match &expr.kind {
                ExprKind::Pipeline(pipeline) => {
                    for expr in &pipeline.exprs {
                        writeln!(f, "{expr}")?;
                    }
                }
                _ => writeln!(f, "{}", expr)?,
            },
            StmtKind::FuncDef(func_def) => {
                write!(f, "func {}", func_def.name)?;
                for arg in &func_def.positional_params {
                    write!(f, " {}", arg.name)?;
                }
                for arg in &func_def.named_params {
                    write!(f, " {}", arg.name)?;
                }
                write!(f, " -> {}\n\n", func_def.body)?;
            }
            StmtKind::TableDef(table) => {
                let pipeline = &table.value;
                match &pipeline.kind {
                    ExprKind::FuncCall(_) => {
                        write!(f, "table {} = (\n  {pipeline}\n)\n\n", table.name)?;
                    }

                    _ => {
                        write!(f, "table {} = {pipeline}\n\n", table.name)?;
                    }
                };
            }
        }
        Ok(())
    }
}
