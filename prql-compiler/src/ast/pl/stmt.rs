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
    pub name: String,
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
    TypeDef(TypeDef),
    ModuleDef(ModuleDef),
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
    pub positional_params: Vec<FuncParam>, // ident
    pub named_params: Vec<FuncParam>,      // named expr
    pub body: Box<Expr>,
    pub return_ty: Option<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FuncParam {
    pub name: String,

    /// Parsed expression that will be resolved to a type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty_expr: Option<Expr>,

    pub default_value: Option<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct VarDef {
    pub value: Box<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct TypeDef {
    pub value: Option<Expr>,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ModuleDef {
    pub stmts: Vec<Stmt>,
}

impl From<StmtKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(_: StmtKind) -> Self {
        anyhow!("Failed to convert statement")
    }
}

impl Display for Statements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for stmt in &self.0 {
            write!(f, "{}", stmt)?;
            write!(f, "\n\n")?;
        }
        Ok(())
    }
}

impl Display for Stmt {
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
            StmtKind::Main(expr) => match &expr.kind {
                ExprKind::Pipeline(pipeline) => {
                    for expr in &pipeline.exprs {
                        writeln!(f, "{expr}")?;
                    }
                }
                _ => writeln!(f, "{}", expr)?,
            },
            StmtKind::FuncDef(func_def) => {
                write!(f, "func {}", self.name)?;
                for arg in &func_def.positional_params {
                    write!(f, " {}", arg.name)?;
                }
                for arg in &func_def.named_params {
                    write!(f, " {}:{}", arg.name, arg.default_value.as_ref().unwrap())?;
                }
                writeln!(f, " -> {}", func_def.body)?;
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
