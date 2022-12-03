use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

use super::module::{Module, NS_DEFAULT_DB, NS_NO_RESOLVE, NS_SELF, NS_STD};
use crate::ast::pl::*;
use crate::error::Span;

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) root_mod: Module,

    pub(crate) span_map: HashMap<usize, Span>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Decl {
    pub declared_at: Option<usize>,

    pub kind: DeclKind,
}

#[derive(Debug, Serialize, Deserialize, Clone, EnumAsInner)]
pub enum DeclKind {
    /// A nested namespace
    Module(Module),

    /// Nested namespaces that do lookup in layers from top to bottom, stoping at first match.
    LayeredModules(Vec<Module>),

    TableDecl(TableDecl),

    InstanceOf(Ident),

    Column(usize),

    /// Contains a default value to be created in parent namespace matched.
    Wildcard(Box<DeclKind>),

    FuncDef(FuncDef),

    Expr(Box<Expr>),

    NoResolve,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableDecl {
    /// Columns layout
    pub frame: TableFrame,

    /// None means that this is an extern table (actual table in database)
    /// Some means a CTE
    pub expr: Option<Box<Expr>>,
}

#[derive(Clone, Default, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableFrame {
    pub columns: Vec<TableColumn>,
}

#[derive(Clone, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub enum TableColumn {
    Wildcard,
    Single(Option<String>),
}

impl Context {
    pub fn declare_func(&mut self, func_def: FuncDef, id: Option<usize>) {
        let name = func_def.name.clone();

        let path = vec![NS_STD.to_string()];
        let ident = Ident { name, path };

        let decl = Decl {
            kind: DeclKind::FuncDef(func_def),
            declared_at: id,
        };
        self.root_mod.insert(ident, decl).unwrap();
    }

    pub fn declare_table(&mut self, table_def: TableDef, id: Option<usize>) {
        let name = table_def.name;
        let path = vec![NS_DEFAULT_DB.to_string()];
        let ident = Ident { name, path };

        let frame = table_def.value.ty.clone().unwrap().into_table().unwrap();
        let frame = TableFrame {
            columns: (frame.columns.into_iter())
                .map(|col| match col {
                    FrameColumn::Wildcard { .. } => TableColumn::Wildcard,
                    FrameColumn::Single { name, .. } => TableColumn::Single(name.map(|n| n.name)),
                })
                .collect(),
        };

        let expr = Some(table_def.value);
        let decl = Decl {
            declared_at: id,
            kind: DeclKind::TableDecl(TableDecl { frame, expr }),
        };

        self.root_mod.insert(ident, decl).unwrap();
    }

    pub fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, String> {
        // lookup the name
        if ident.name != "*" {
            let decls = self.root_mod.lookup(ident);

            match decls.len() {
                // no match: try match *
                0 => {}

                // single match, great!
                1 => return Ok(decls.into_iter().next().unwrap()),

                // ambiguous
                _ => {
                    let decls = decls.into_iter().map(|d| d.to_string()).join(", ");
                    return Err(format!("Ambiguous reference. Could be from any of {decls}"));
                }
            }
        }

        // this variable can be from a namespace that we don't know all columns of
        let decls = self.root_mod.lookup(&Ident {
            path: ident.path.clone(),
            name: "*".to_string(),
        });

        match decls.len() {
            0 => Err(format!("Unknown name {ident}")),

            // single match, great!
            1 => {
                let wildcard_ident = decls.into_iter().next().unwrap();

                let wildcard = self.root_mod.get(&wildcard_ident).unwrap();
                let wildcard_default = wildcard.kind.as_wildcard().cloned().unwrap();

                let module_ident = wildcard_ident.pop().unwrap();
                let module = self.root_mod.get_mut(&module_ident).unwrap();
                let module = module.kind.as_module_mut().unwrap();

                // insert default
                module
                    .names
                    .insert(ident.name.clone(), Decl::from(*wildcard_default));

                // table columns
                if let Some(decl) = module.names.get(NS_SELF).cloned() {
                    if let DeclKind::InstanceOf(table_ident) = decl.kind {
                        log::debug!("infering {ident} to be from table {table_ident}");
                        self.infer_table_column(&table_ident, &ident.name)?;
                    }
                }

                Ok(module_ident + Ident::from_name(ident.name.clone()))
            }

            // don't report ambiguous variable, database may be able to resolve them
            _ => {
                // insert default
                let ident = NS_NO_RESOLVE.to_string();
                self.root_mod
                    .names
                    .insert(ident, Decl::from(DeclKind::NoResolve));

                log::debug!(
                    "... could either of {:?}",
                    decls.iter().map(|x| x.to_string()).collect_vec()
                );

                Ok(Ident::from_name(NS_NO_RESOLVE))
            }
        }
    }

    fn infer_table_column(&mut self, table_ident: &Ident, col_name: &str) -> Result<(), String> {
        let table = self.root_mod.get_mut(table_ident).unwrap();
        let table_decl = table.kind.as_table_decl_mut().unwrap();

        let has_wildcard =
            (table_decl.frame.columns.iter()).any(|c| matches!(c, TableColumn::Wildcard));
        if !has_wildcard {
            return Err(format!("Table {table_ident:?} does not have wildcard."));
        }

        let exists = table_decl.frame.columns.iter().any(|c| match c {
            TableColumn::Single(Some(n)) => n == col_name,
            _ => false,
        });
        if exists {
            return Ok(());
        }

        let col = TableColumn::Single(Some(col_name.to_string()));
        table_decl.frame.columns.push(col);

        // also add into input tables of this table expression
        if let Some(expr) = &table_decl.expr {
            if let Some(Ty::Table(frame)) = expr.ty.as_ref() {
                let wildcard_inputs = (frame.columns.iter())
                    .filter_map(|c| c.as_wildcard())
                    .collect_vec();

                match wildcard_inputs.len() {
                    0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
                    1 => {
                        let input_name = wildcard_inputs.into_iter().next().unwrap();

                        let input = frame.find_input(input_name).unwrap();
                        if let Some(table_ident) = input.table.clone() {
                            self.infer_table_column(&table_ident, col_name)?;
                        }
                    }
                    _ => {
                        return Err(format!("Cannot infer where {table_ident}.{col_name} is from. It could be any of {wildcard_inputs:?}"))
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for DeclKind {
    fn default() -> Self {
        DeclKind::Module(Module::default())
    }
}

impl Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.root_mod.fmt(f)
    }
}

impl From<DeclKind> for Decl {
    fn from(kind: DeclKind) -> Self {
        Decl {
            kind,
            declared_at: None,
        }
    }
}

impl std::fmt::Display for Decl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.kind, f)
    }
}

impl std::fmt::Display for DeclKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Module(arg0) => f.debug_tuple("Module").field(arg0).finish(),
            Self::LayeredModules(arg0) => f.debug_tuple("LayeredModules").field(arg0).finish(),
            Self::TableDecl(TableDecl { frame, expr }) => write!(f, "TableDef: {frame} {expr:?}"),
            Self::InstanceOf(arg0) => write!(f, "InstanceOf: {arg0}"),
            Self::Column(arg0) => write!(f, "Column (target {arg0})"),
            Self::Wildcard(arg0) => write!(f, "Wildcard (default: {arg0})"),
            Self::FuncDef(arg0) => write!(f, "FuncDef: {arg0}"),
            Self::Expr(arg0) => write!(f, "Expr: {arg0}"),
            Self::NoResolve => write!(f, "NoResolve"),
        }
    }
}

impl std::fmt::Display for TableFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for (index, col) in self.columns.iter().enumerate() {
            let is_last = index == self.columns.len() - 1;

            let col = match col {
                TableColumn::Wildcard => "*",
                TableColumn::Single(name) => name.as_deref().unwrap_or("<unnamed>"),
            };
            f.write_str(col)?;
            if !is_last {
                f.write_str(", ")?;
            }
        }
        f.write_str("]")
    }
}
