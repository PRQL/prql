use anyhow::Result;
use enum_as_inner::EnumAsInner;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug};

use super::*;
use crate::error::Span;
use crate::ir::pl::*;

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
            Self::Expr(arg0) => write!(f, "Expr: {}", write_pl(*arg0.clone())),
            Self::QueryDef(_) => write!(f, "QueryDef"),
        }
    }
}

fn is_zero(x: &usize) -> bool {
    *x == 0
}
