use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::{collections::HashMap, fmt::Debug};

use super::*;
use crate::ast::pl::*;
use crate::error::{Error, Span};

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) root_mod: Module,

    pub(crate) span_map: HashMap<usize, Span>,
}

/// A struct containing information about a single declaration.
#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct Decl {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_at: Option<usize>,

    pub kind: DeclKind,

    /// Some declarations (like relation columns) have an order to them.
    /// 0 means that the order is irrelevant.
    #[serde(skip_serializing_if = "is_zero")]
    pub order: usize,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
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

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct TableDecl {
    /// This will always be `TyKind::Array(TyKind::Tuple)`.
    /// It is being preparing to be merged with [DeclKind::Expr].
    /// It used to keep track of columns.
    pub ty: Option<Ty>,

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
    pub fn declare(
        &mut self,
        ident: Ident,
        decl: DeclKind,
        id: Option<usize>,
        annotations: Vec<Annotation>,
    ) -> Result<()> {
        let existing = self.root_mod.get(&ident);
        if existing.is_some() {
            return Err(Error::new_simple(format!("duplicate declarations of {ident}")).into());
        }

        let decl = Decl {
            kind: decl,
            declared_at: id,
            order: 0,
            annotations,
        };
        self.root_mod.insert(ident, decl).unwrap();
        Ok(())
    }

    pub fn prepare_expr_decl(&mut self, value: Box<Expr>) -> DeclKind {
        match &value.lineage {
            Some(frame) => {
                let columns = (frame.columns.iter())
                    .map(|col| match col {
                        LineageColumn::All { .. } => TupleField::Wildcard(None),
                        LineageColumn::Single { name, .. } => {
                            TupleField::Single(name.as_ref().map(|n| n.name.clone()), None)
                        }
                    })
                    .collect();
                let ty = Some(Ty::relation(columns));

                let expr = TableExpr::RelationVar(value);
                DeclKind::TableDecl(TableDecl { ty, expr })
            }
            _ => DeclKind::Expr(value),
        }
    }

    pub fn resolve_ident(
        &mut self,
        ident: &Ident,
        default_namespace: Option<&String>,
    ) -> Result<Ident, Error> {
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
            return self
                .resolve_ident_wildcard(ident)
                .map_err(Error::new_simple);
        }

        // base case: direct lookup
        let decls = self.root_mod.lookup(ident);
        match decls.len() {
            // no match: try match *
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap()),

            // ambiguous
            _ => return Err(ambiguous_error(decls, None)),
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
                _ => return Err(ambiguous_error(decls, None)),
            }
        } else {
            ident.clone()
        };

        // fallback case: try to match with NS_INFER and infer the declaration from the original ident.
        match self.resolve_ident_fallback(ident, NS_INFER) {
            // The declaration and all needed parent modules were created
            // -> just return the fq ident
            Ok(inferred_ident) => Ok(inferred_ident),

            // Was not able to infer.
            Err(None) => Err(Error::new_simple("Unknown name".to_string())),
            Err(Some(msg)) => Err(msg),
        }
    }

    /// Try lookup of the ident with name replaced. If unsuccessful, recursively retry parent ident.
    fn resolve_ident_fallback(
        &mut self,
        ident: Ident,
        name_replacement: &'static str,
    ) -> Result<Ident, Option<Error>> {
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

        match decls.len() {
            1 => {
                // single match, great!
                let infer_ident = decls.into_iter().next().unwrap();
                self.infer_decl(infer_ident, &ident)
                    .map_err(|x| Some(Error::new_simple(x)))
            }
            0 => Err(None),
            _ => Err(Some(ambiguous_error(decls, Some(&ident.name)))),
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
                // Ident could be just part of NS_THIS
                let mod_ident = (Ident::from_name(NS_THIS) + ident.clone()).pop().unwrap();

                if let Some(mod_decl) = self.root_mod.get_mut(&mod_ident) {
                    (mod_ident, mod_decl)
                } else {
                    // ... or part of NS_THAT
                    let mod_ident = (Ident::from_name(NS_THAT) + ident.clone()).pop().unwrap();

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
            vec![Expr::new(ExprKind::All {
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
                .map(|fq_col| Expr::new(ExprKind::Ident(fq_col)))
                .collect_vec()
        };

        // This is just a workaround to return an Expr from this function.
        // We wrap the expr into DeclKind::Expr and save it into context.
        let cols_expr = Expr {
            flatten: true,
            ..Expr::new(ExprKind::Tuple(fq_cols))
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

        let Some(columns) = table_decl.ty.as_mut().and_then(|t| t.as_relation_mut()) else {
            return Err(format!("Variable {table_ident:?} is not a relation."));
        };

        let has_wildcard = columns.iter().any(|c| matches!(c, TupleField::Wildcard(_)));
        if !has_wildcard {
            return Err(format!("Table {table_ident:?} does not have wildcard."));
        }

        let exists = columns.iter().any(|c| match c {
            TupleField::Single(Some(n), _) => n == col_name,
            _ => false,
        });
        if exists {
            return Ok(());
        }

        columns.push(TupleField::Single(Some(col_name.to_string()), None));

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

fn ambiguous_error(idents: HashSet<Ident>, replace_name: Option<&String>) -> Error {
    let all_this = idents.iter().all(|d| d.starts_with_part(NS_THIS));

    let mut chunks = Vec::new();
    for mut ident in idents {
        if all_this {
            let (_, rem) = ident.pop_front();
            if let Some(rem) = rem {
                ident = rem;
            } else {
                continue;
            }
        }

        if let Some(name) = replace_name {
            ident.name = name.clone();
        }
        chunks.push(ident.to_string());
    }
    chunks.sort();
    let hint = format!("could be any of: {}", chunks.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
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
            annotations: Vec::new(),
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
            Self::TableDecl(TableDecl { ty, expr }) => {
                write!(
                    f,
                    "TableDecl: {} {expr:?}",
                    ty.as_ref().map(|t| t.to_string()).unwrap_or_default()
                )
            }
            Self::InstanceOf(arg0) => write!(f, "InstanceOf: {arg0}"),
            Self::Column(arg0) => write!(f, "Column (target {arg0})"),
            Self::Infer(arg0) => write!(f, "Infer (default: {arg0})"),
            Self::Expr(arg0) => write!(f, "Expr: {arg0}"),
            Self::QueryDef(_) => write!(f, "QueryDef"),
        }
    }
}

fn is_zero(x: &usize) -> bool {
    *x == 0
}
