use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

use super::module::Module;
use super::{
    NS_DEFAULT_DB, NS_FRAME, NS_FRAME_RIGHT, NS_INFER, NS_INFER_MODULE, NS_MAIN, NS_QUERY_DEF,
    NS_SELF,
};
use crate::ast::pl::*;
use crate::ast::rq::RelationColumn;
use crate::error::{Error, Span};

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) root_mod: Module,

    pub(crate) span_map: HashMap<usize, Span>,

    pub(crate) options: ResolverOptions,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct ResolverOptions {
    pub allow_module_decls: bool,
}

/// A struct containing information about a single declaration.
#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct Decl {
    pub declared_at: Option<usize>,

    pub kind: DeclKind,

    /// Some declarations (like relation columns) have an order to them.
    /// 0 means that the order is irrelevant.
    pub order: usize,
}

/// The Declaration itself.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, EnumAsInner)]
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

    Expr(Box<Expr>),

    QueryDef(QueryDef),
}

#[derive(PartialEq, Serialize, Deserialize, Clone)]
pub struct TableDecl {
    /// Columns layout
    pub columns: Vec<RelationColumn>,

    pub expr: TableExpr,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, EnumAsInner)]
pub enum TableExpr {
    /// In SQL, this is a CTE
    RelationVar(Box<Expr>),

    /// Actual table in a database. In SQL it can be referred to by name.
    LocalTable,

    /// No expression (this decl just tracks a relation literal).
    None,

    /// A placeholder for a relation that will be provided later.
    Param(String),
}

#[derive(Clone, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub enum TableColumn {
    Wildcard,
    Single(Option<String>),
}

impl Context {
    pub fn declare(&mut self, ident: Ident, decl: DeclKind, id: Option<usize>) -> Result<()> {
        let existing = self.root_mod.get(&ident);
        if existing.is_some() {
            return Err(Error::new_simple(format!("duplicate declarations of {ident}")).into());
        }

        let decl = Decl {
            kind: decl,
            declared_at: id,
            order: 0,
        };
        self.root_mod.insert(ident, decl).unwrap();
        Ok(())
    }

    pub fn prepare_expr_decl(&mut self, value: Box<Expr>) -> DeclKind {
        match &value.lineage {
            Some(frame) => {
                let columns = (frame.columns.iter())
                    .map(|col| match col {
                        LineageColumn::All { .. } => RelationColumn::Wildcard,
                        LineageColumn::Single { name, .. } => {
                            RelationColumn::Single(name.as_ref().map(|n| n.name.clone()))
                        }
                    })
                    .collect();

                let expr = TableExpr::RelationVar(value);
                DeclKind::TableDecl(TableDecl { columns, expr })
            }
            _ => DeclKind::Expr(value),
        }
    }

    pub fn resolve_ident(
        &mut self,
        ident: &Ident,
        default_namespace: Option<&String>,
    ) -> Result<Ident, String> {
        // special case: wildcard
        if ident.name == "*" {
            // TODO: we may want to raise an error if someone has passed `download*` in
            // an attempt to query for all `download` columns and expects to be able
            // to select a `download_2020_01_01` column later in the query. But
            // sometimes we want to query for `*.parquet` files, and give them an
            // alias. So we don't raise an error here, but if there's a way of
            // differentiating the cases, we can implement that.
            // if ident.name != "*" {
            //     return Err("Unsupported feature: advanced wildcard column matching".to_string());
            // }
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
                return Err({
                    let decls = decls.into_iter().map(|d| d.to_string()).join(", ");
                    format!("Ambiguous name. Could be from any of {decls}")
                })
            }
        }

        let ident = if let Some(default_namespace) = default_namespace {
            let ident = ident.clone().prepend(vec![default_namespace.clone()]);

            let decls = self.root_mod.lookup(&ident);
            match decls.len() {
                // no match: try match *
                0 => ident,

                // single match, great!
                1 => return Ok(decls.into_iter().next().unwrap()),

                // ambiguous
                _ => {
                    return Err({
                        let decls = decls.into_iter().map(|d| d.to_string()).join(", ");
                        format!("Ambiguous name. Could be from any of {decls}")
                    })
                }
            }
        } else {
            ident.clone()
        };

        // fallback case: try to match with NS_INFER and infer the declaration from the original ident.
        match self.resolve_ident_fallback(ident, NS_INFER) {
            // The declaration and all needed parent modules were created
            // -> just return the fq ident
            Some(inferred_ident) => Ok(inferred_ident),

            // Was not able to infer.
            None => Err("Unknown name".to_string()),
        }
    }

    /// Try lookup of the ident with name replaced. If unsuccessful, recursively retry parent ident.
    fn resolve_ident_fallback(
        &mut self,
        ident: Ident,
        name_replacement: &'static str,
    ) -> Option<Ident> {
        let infer_ident = ident.clone().with_name(name_replacement);

        // lookup of infer_ident
        let mut decls = self.root_mod.lookup(&infer_ident);

        if decls.is_empty() {
            if let Some(parent) = infer_ident.clone().pop() {
                // try to infer parent
                let _ = self.resolve_ident_fallback(parent, NS_INFER_MODULE)?;

                // module was successfully inferred, retry the lookup
                decls = self.root_mod.lookup(&infer_ident)
            }
        }

        if decls.len() == 1 {
            // single match, great!
            let infer_ident = decls.into_iter().next().unwrap();
            self.infer_decl(infer_ident, &ident).ok()
        } else {
            // no matches or ambiguous
            None
        }
    }

    /// Create a declaration of [original] from template provided by declaration of [infer_ident].
    fn infer_decl(&mut self, infer_ident: Ident, original: &Ident) -> Result<Ident, String> {
        let infer = self.root_mod.get(&infer_ident).unwrap();
        let mut infer_default = *infer.kind.as_infer().cloned().unwrap();

        if let DeclKind::Module(new_module) = &mut infer_default {
            // Modules are inferred only for database inference.
            // Because we want to infer database modules that nested arbitrarily deep,
            // we cannot store the template in DeclKind::Infer, but we override it here.
            *new_module = Module::new_database();
        }

        let module_ident = infer_ident.pop().unwrap();
        let module = self.root_mod.get_mut(&module_ident).unwrap();
        let module = module.kind.as_module_mut().unwrap();

        // insert default
        module
            .names
            .insert(original.name.clone(), Decl::from(infer_default));

        // infer table columns
        if let Some(decl) = module.names.get(NS_SELF).cloned() {
            if let DeclKind::InstanceOf(table_ident) = decl.kind {
                log::debug!("inferring {original} to be from table {table_ident}");
                self.infer_table_column(&table_ident, &original.name)?;
            }
        }

        Ok(module_ident + Ident::from_name(original.name.clone()))
    }

    fn resolve_ident_wildcard(&mut self, ident: &Ident) -> Result<Ident, String> {
        // Try matching ident prefix with a module
        let (mod_ident, mod_decl) = {
            if ident.path.len() > 1 {
                // Ident has specified full path
                let mod_ident = ident.clone().pop().unwrap();
                let mod_decl = (self.root_mod.get_mut(&mod_ident))
                    .ok_or_else(|| format!("Unknown relation {ident}"))?;

                (mod_ident, mod_decl)
            } else {
                // Ident could be just part of NS_FRAME
                let mod_ident = (Ident::from_name(NS_FRAME) + ident.clone()).pop().unwrap();

                if let Some(mod_decl) = self.root_mod.get_mut(&mod_ident) {
                    (mod_ident, mod_decl)
                } else {
                    // ... or part of NS_FRAME_RIGHT
                    let mod_ident = (Ident::from_name(NS_FRAME_RIGHT) + ident.clone())
                        .pop()
                        .unwrap();

                    let mod_decl = self.root_mod.get_mut(&mod_ident);

                    // ... well - I guess not. Throw.
                    let mod_decl = mod_decl.ok_or_else(|| format!("Unknown relation {ident}"))?;

                    (mod_ident, mod_decl)
                }
            }
        };

        // Unwrap module
        let module = (mod_decl.kind.as_module_mut())
            .ok_or_else(|| format!("Expected a module {mod_ident}"))?;

        let fq_cols = if module.names.contains_key(NS_INFER) {
            // Columns can be inferred, which means that we don't know all column names at
            // compile time: use ExprKind::All
            vec![Expr::from(ExprKind::All {
                within: mod_ident.clone(),
                except: Vec::new(),
            })]
        } else {
            // Columns cannot be inferred, what's in the namespace is all there
            // could be in this namespace.
            (module.names.iter())
                .filter(|(_, decl)| matches!(&decl.kind, DeclKind::Column(_)))
                .sorted_by_key(|(_, decl)| decl.order)
                .map(|(name, _)| mod_ident.clone() + Ident::from_name(name))
                .map(|fq_col| Expr::from(ExprKind::Ident(fq_col)))
                .collect_vec()
        };

        // This is just a workaround to return an Expr from this function.
        // We wrap the expr into DeclKind::Expr and save it into context.
        let cols_expr = Expr {
            flatten: true,
            ..Expr::from(ExprKind::Tuple(fq_cols))
        };
        let cols_expr = DeclKind::Expr(Box::new(cols_expr));
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
        if let TableExpr::RelationVar(expr) = &table_decl.expr {
            if let Some(frame) = &expr.lineage {
                let wildcard_inputs = (frame.columns.iter())
                    .filter_map(|c| c.as_all())
                    .collect_vec();

                match wildcard_inputs.len() {
                    0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
                    1 => {
                        let (input_name, _) = wildcard_inputs.into_iter().next().unwrap();

                        let input = frame.find_input(input_name).unwrap();
                        let table_ident = input.table.clone();
                        self.infer_table_column(&table_ident, col_name)?;
                    }
                    _ => {
                        return Err(format!("Cannot infer where {table_ident}.{col_name} is from. It could be any of {wildcard_inputs:?}"))
                    }
                }
            }
        }

        Ok(())
    }

    /// Converts a identifier that points to a table declaration to a frame of that table.
    pub fn table_decl_to_frame(
        &self,
        table_fq: &Ident,
        input_name: String,
        input_id: usize,
    ) -> Lineage {
        let id = input_id;
        let table_decl = self.root_mod.get(table_fq).unwrap();
        let TableDecl { columns, .. } = table_decl.kind.as_table_decl().unwrap();

        let instance_frame = Lineage {
            inputs: vec![LineageInput {
                id,
                name: input_name.clone(),
                table: table_fq.clone(),
            }],
            columns: columns
                .iter()
                .map(|col| match col {
                    RelationColumn::Wildcard => LineageColumn::All {
                        input_name: input_name.clone(),
                        except: columns
                            .iter()
                            .flat_map(|c| c.as_single().cloned().flatten())
                            .collect(),
                    },
                    RelationColumn::Single(col_name) => LineageColumn::Single {
                        name: col_name
                            .clone()
                            .map(|col_name| Ident::from_path(vec![input_name.clone(), col_name])),
                        expr_id: id,
                    },
                })
                .collect(),
            ..Default::default()
        };

        log::debug!("instanced table {table_fq} as {instance_frame:?}");
        instance_frame
    }

    /// Declares a new table for a relation literal.
    /// This is needed for column inference to work properly.
    pub fn declare_table_for_literal(
        &mut self,
        input_id: usize,
        columns: Option<Vec<RelationColumn>>,
        name_hint: Option<String>,
    ) -> Lineage {
        let id = input_id;
        let global_name = format!("_literal_{}", id);

        // declare a new table in the `default_db` module
        let default_db_ident = Ident::from_name(NS_DEFAULT_DB);
        let default_db = self.root_mod.get_mut(&default_db_ident).unwrap();
        let default_db = default_db.kind.as_module_mut().unwrap();

        let infer_default = default_db.get(&Ident::from_name(NS_INFER)).unwrap().clone();
        let mut infer_default = *infer_default.kind.into_infer().unwrap();

        let table_decl = infer_default.as_table_decl_mut().unwrap();
        table_decl.expr = TableExpr::None;

        if let Some(columns) = columns {
            table_decl.columns = columns;
        }

        default_db
            .names
            .insert(global_name.clone(), Decl::from(infer_default));

        // produce a frame of that table
        let input_name = name_hint.unwrap_or_else(|| global_name.clone());
        let table_fq = default_db_ident + Ident::from_name(global_name);
        self.table_decl_to_frame(&table_fq, input_name, id)
    }

    /// Finds that main pipeline given a path to either main itself or its parent module.
    /// Returns main expr and fq ident of the decl.
    pub fn find_main_rel(&self, path: &[String]) -> Result<(&TableExpr, Ident), Option<String>> {
        let (decl, ident) = self.find_main(path)?;

        let decl = (decl.kind.as_table_decl())
            .ok_or(Some(format!("{ident} is not a relational variable")))?;

        Ok((&decl.expr, ident))
    }

    pub fn find_main(&self, path: &[String]) -> Result<(&Decl, Ident), Option<String>> {
        let mut tried_idents = Vec::new();

        // is path referencing the relational var directly?
        if !path.is_empty() {
            let ident = Ident::from_path(path.to_vec());
            let decl = self.root_mod.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        // is path referencing the parent module?
        {
            let mut path = path.to_vec();
            path.push(NS_MAIN.to_string());

            let ident = Ident::from_path(path);
            let decl = self.root_mod.get(&ident);

            if let Some(decl) = decl {
                return Ok((decl, ident));
            } else {
                tried_idents.push(ident.to_string());
            }
        }

        Err(Some(format!(
            "Expected a declaration at {}",
            tried_idents.join(" or ")
        )))
    }

    pub fn find_query_def(&self, main: &Ident) -> Option<&QueryDef> {
        let ident = Ident {
            path: main.path.clone(),
            name: NS_QUERY_DEF.to_string(),
        };

        let decl = self.root_mod.get(&ident)?;
        decl.kind.as_query_def()
    }

    /// Finds all main pipelines.
    pub fn find_mains(&self) -> Vec<Ident> {
        self.root_mod.find_by_suffix(NS_MAIN)
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
                write!(f, "TableDecl: {} {expr:?}", RelationColumns(columns))
            }
            Self::InstanceOf(arg0) => write!(f, "InstanceOf: {arg0}"),
            Self::Column(arg0) => write!(f, "Column (target {arg0})"),
            Self::Infer(arg0) => write!(f, "Infer (default: {arg0})"),
            Self::Expr(arg0) => write!(f, "Expr: {arg0}"),
            Self::QueryDef(_) => write!(f, "QueryDef"),
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

impl std::fmt::Debug for TableDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(self).unwrap();
        let json = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        f.write_str(&serde_yaml::to_string(&json).unwrap())
    }
}
