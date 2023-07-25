use std::fmt::Display;

use anyhow::{anyhow, bail};

use crate::ir::pl::{Annotation, ExprKind, Stmt, StmtKind};

impl Annotation {
    /// Find the items in a `@{a=b}`. We're only using annotations with tuples;
    /// we can consider formalizing this constraint.
    pub fn tuple_items(self) -> anyhow::Result<Vec<(String, ExprKind)>> {
        match self.expr.kind {
            ExprKind::Tuple(items) => items
                .into_iter()
                .map(|item| Ok((item.alias.clone().unwrap(), item.kind)))
                .collect(),
            _ => bail!("Annotation must be a tuple"),
        }
    }
}

impl From<StmtKind> for anyhow::Error {
    // https://github.com/bluejekyll/enum-as-inner/issues/84
    #[allow(unreachable_code)]
    fn from(_: StmtKind) -> Self {
        anyhow!("Failed to convert statement")
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
            StmtKind::VarDef(var) => {
                let pipeline = &var.value;
                match &pipeline.kind {
                    ExprKind::FuncCall(_) => {
                        write!(f, "let {} = (\n  {pipeline}\n)\n\n", self.name())?;
                    }

                    _ => {
                        write!(f, "let {} = {pipeline}\n\n", self.name())?;
                    }
                };
            }
            StmtKind::TypeDef(ty_def) => {
                if let Some(value) = &ty_def.value {
                    write!(f, "type {} = {value}\n\n", self.name())?;
                } else {
                    write!(f, "type {}\n\n", self.name())?;
                }
            }
            StmtKind::ModuleDef(module_def) => {
                write!(f, "module {} {{", self.name())?;
                for stmt in &module_def.stmts {
                    write!(f, "{}", stmt)?;
                }
                write!(f, "}}\n\n")?;
            }
        }
        Ok(())
    }
}
