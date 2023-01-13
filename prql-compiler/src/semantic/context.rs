use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::{collections::HashMap, fmt::Debug};

use super::module::{Module, NS_DEFAULT_DB, NS_FRAME, NS_FRAME_RIGHT, NS_INFER, NS_SELF, NS_STD};
use super::type_resolver::validate_type;
use crate::ast::pl::*;
use crate::ast::rq::RelationColumn;
use crate::error::{Error, Span};

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) root_mod: Module,

    pub(crate) span_map: HashMap<usize, Span>,

    pub(crate) inferred_columns: HashMap<usize, Vec<RelationColumn>>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Decl {
    pub declared_at: Option<usize>,

    pub kind: DeclKind,

    /// Some declarations (like relation columns) have an order to them.
    /// 0 means that the order is irrelevant.
    pub order: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone, EnumAsInner)]
pub enum DeclKind {
    /// A nested namespace
    Module(Module),

    /// Nested namespaces that do lookup in layers from top to bottom, stopping at first match.
    LayeredModules(Vec<Module>),

    TableDecl(TableDecl),

    InstanceOf(Ident),

    /// A single column. Contains id of target which is either:
    /// - an input relation that is source of this column or
    /// - a column expression.
    Column(usize),

    /// Contains a default value to be created in parent namespace when NS_INFER is matched.
    Infer(Box<DeclKind>),

    FuncDef(FuncDef),

    Expr(Box<Expr>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableDecl {
    /// Columns layout
    pub columns: Vec<RelationColumn>,

    /// None means that this is an extern table (actual table in database)
    /// Some means a CTE
    pub expr: Option<Box<Expr>>,
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
            order: 0,
        };
        self.root_mod.insert(ident, decl).unwrap();
    }

    pub fn declare_var(
        &mut self,
        var_def: VarDef,
        id: Option<usize>,
        span: Option<Span>,
    ) -> Result<()> {
        let name = var_def.name;
        let mut path = Vec::new();

        let decl = match &var_def.value.ty {
            Some(Ty::Table(_) | Ty::Infer) => {
                let mut value = var_def.value;

                let ty = value.ty.clone().unwrap();
                let frame = ty.into_table().unwrap_or_else(|_| {
                    let assumed =
                        validate_type(value.as_ref(), &Ty::Table(Frame::default()), || None)
                            .unwrap();
                    value.ty = Some(assumed.clone());
                    assumed.into_table().unwrap()
                });

                path = vec![NS_DEFAULT_DB.to_string()];

                let columns = (frame.columns.iter())
                    .map(|col| match col {
                        FrameColumn::All { .. } => RelationColumn::Wildcard,
                        FrameColumn::Single { name, .. } => {
                            RelationColumn::Single(name.as_ref().map(|n| n.name.clone()))
                        }
                    })
                    .collect();

                let expr = Some(value);
                DeclKind::TableDecl(TableDecl { columns, expr })
            }
            Some(_) => DeclKind::Expr(var_def.value),
            None => {
                return Err(
                    Error::new_simple("Cannot infer type. Type annotations needed.")
                        .with_span(span)
                        .into(),
                );
            }
        };

        let decl = Decl {
            declared_at: id,
            kind: decl,
            order: 0,
        };

        let ident = Ident { name, path };
        self.root_mod.insert(ident, decl).unwrap();

        Ok(())
    }

    pub fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, String> {
        // special case: wildcard
        if ident.name.contains('*') {
            return self.resolve_ident_wildcard(ident);
        }

        // base case: direct lookup
        let decls = self.root_mod.lookup(ident);
        match decls.len() {
            // no match: try match *
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap()),

            // ambiguous
            _ => {
                let decls = decls.into_iter().map(|d| d.to_string()).join(", ");
                return Err(format!("Ambiguous name. Could be from any of {decls}"));
            }
        }

        // fallback case: this variable can be from a namespace that we don't know all columns of
        let decls = if ident.name != "*" {
            self.root_mod.lookup(&Ident {
                path: ident.path.clone(),
                name: NS_INFER.to_string(),
            })
        } else {
            HashSet::new()
        };
        match decls.len() {
            0 => Err(format!("Unknown name {ident}")),

            // single match, great!
            1 => {
                let infer_ident = decls.into_iter().next().unwrap();

                let infer = self.root_mod.get(&infer_ident).unwrap();
                let infer_default = infer.kind.as_infer().cloned().unwrap();
                let input_id = infer.declared_at;

                let module_ident = infer_ident.pop().unwrap();
                let module = self.root_mod.get_mut(&module_ident).unwrap();
                let module = module.kind.as_module_mut().unwrap();

                // insert default
                module
                    .names
                    .insert(ident.name.clone(), Decl::from(*infer_default));

                // infer table columns
                if let Some(decl) = module.names.get(NS_SELF).cloned() {
                    if let DeclKind::InstanceOf(table_ident) = decl.kind {
                        log::debug!("inferring {ident} to be from table {table_ident}");
                        self.infer_table_column(&table_ident, &ident.name)?;
                    }
                }

                // for inline expressions with wildcards (s-strings), we cannot store inferred columns
                // in global namespace, but still need the information for lowering.
                // as a workaround, we store it in context directly.
                if let Some(input_id) = input_id {
                    let inferred = self.inferred_columns.entry(input_id).or_default();

                    let exists = inferred.iter().any(|c| match c {
                        RelationColumn::Single(Some(name)) => name == &ident.name,
                        _ => false,
                    });
                    if !exists {
                        inferred.push(RelationColumn::Single(Some(ident.name.clone())));
                    }
                }

                Ok(module_ident + Ident::from_name(ident.name.clone()))
            }

            // ambiguous
            _ => {
                let decls = decls.into_iter().map(|d| d.to_string()).join(", ");
                Err(format!("Ambiguous name. Could be from any of {decls}"))
            }
        }
    }

    fn resolve_ident_wildcard(&mut self, ident: &Ident) -> Result<Ident, String> {
        if ident.name != "*" {
            return Err("Unsupported feature: advanced wildcard column matching".to_string());
        }

        let (mod_ident, mod_decl) = {
            if ident.path.len() > 1 {
                let mod_ident = ident.clone().pop().unwrap();
                let mod_decl = (self.root_mod.get_mut(&mod_ident))
                    .ok_or_else(|| format!("Unknown relation {ident}"))?;

                (mod_ident, mod_decl)
            } else {
                let mod_ident = (Ident::from_name(NS_FRAME) + ident.clone()).pop().unwrap();

                if let Some(mod_decl) = self.root_mod.get_mut(&mod_ident) {
                    (mod_ident, mod_decl)
                } else {
                    let mod_ident = (Ident::from_name(NS_FRAME_RIGHT) + ident.clone())
                        .pop()
                        .unwrap();

                    let mod_decl = (self.root_mod.get_mut(&mod_ident))
                        .ok_or_else(|| format!("Unknown relation {ident}"))?;

                    (mod_ident, mod_decl)
                }
            }
        };

        let module = (mod_decl.kind.as_module_mut())
            .ok_or_else(|| format!("Expected a module {mod_ident}"))?;

        let fq_cols = if module.names.contains_key(NS_INFER) {
            vec![Expr::from(ExprKind::All {
                within: mod_ident.clone(),
                except: Vec::new(),
            })]
        } else {
            (module.names.iter())
                .filter(|(_, decl)| matches!(&decl.kind, DeclKind::Column(_)))
                .sorted_by_key(|(_, decl)| decl.order)
                .map(|(name, _)| mod_ident.clone() + Ident::from_name(name))
                .map(|fq_col| Expr::from(ExprKind::Ident(fq_col)))
                .collect_vec()
        };

        // This is just a workaround to return an Expr from this function.
        // We wrap the expr into DeclKind::Expr and save it into context.
        let cols_expr = DeclKind::Expr(Box::new(Expr::from(ExprKind::List(fq_cols))));
        let save_as = "_wildcard_match";
        module.names.insert(save_as.to_string(), cols_expr.into());

        // Then we can return ident to that decl.
        Ok(mod_ident + Ident::from_name(save_as))
    }

    fn infer_table_column(&mut self, table_ident: &Ident, col_name: &str) -> Result<(), String> {
        let table = self.root_mod.get_mut(table_ident).unwrap();
        let table_decl = table.kind.as_table_decl_mut().unwrap();

        let has_wildcard =
            (table_decl.columns.iter()).any(|c| matches!(c, RelationColumn::Wildcard));
        if !has_wildcard {
            return Err(format!("Table {table_ident:?} does not have wildcard."));
        }

        let exists = table_decl.columns.iter().any(|c| match c {
            RelationColumn::Single(Some(n)) => n == col_name,
            _ => false,
        });
        if exists {
            return Ok(());
        }

        let col = RelationColumn::Single(Some(col_name.to_string()));
        table_decl.columns.push(col);

        // also add into input tables of this table expression
        if let Some(expr) = &table_decl.expr {
            if let Some(Ty::Table(frame)) = expr.ty.as_ref() {
                let wildcard_inputs = (frame.columns.iter())
                    .filter_map(|c| c.as_all())
                    .collect_vec();

                match wildcard_inputs.len() {
                    0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
                    1 => {
                        let (input_name, _) = wildcard_inputs.into_iter().next().unwrap();

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
            order: 0,
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
            Self::TableDecl(TableDecl { columns, expr }) => {
                write!(f, "TableDef: {} {expr:?}", RelationColumns(columns))
            }
            Self::InstanceOf(arg0) => write!(f, "InstanceOf: {arg0}"),
            Self::Column(arg0) => write!(f, "Column (target {arg0})"),
            Self::Infer(arg0) => write!(f, "Infer (default: {arg0})"),
            Self::FuncDef(arg0) => write!(f, "FuncDef: {arg0}"),
            Self::Expr(arg0) => write!(f, "Expr: {arg0}"),
        }
    }
}

pub struct RelationColumns<'a>(pub &'a [RelationColumn]);

impl<'a> std::fmt::Display for RelationColumns<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        for (index, col) in self.0.iter().enumerate() {
            let is_last = index == self.0.len() - 1;

            let col = match col {
                RelationColumn::Wildcard => "*",
                RelationColumn::Single(name) => name.as_deref().unwrap_or("<unnamed>"),
            };
            f.write_str(col)?;
            if !is_last {
                f.write_str(", ")?;
            }
        }
        write!(f, "]")
    }
}
