use std::collections::HashMap;
use std::fmt::Debug;

use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::codegen::write_ty;
use crate::ir::pl;
use crate::pr::{self, Span, Ty};
use crate::semantic::write_pl;

/// Context of the pipeline.
#[derive(Default, Serialize, Deserialize, Clone)]
pub struct RootModule {
    /// Map of all accessible names (for each namespace)
    pub module: Module,

    pub span_map: HashMap<usize, Span>,
}

#[derive(Default, PartialEq, Serialize, Deserialize, Clone)]
pub struct Module {
    /// Names declared in this module. This is the important thing.
    pub names: HashMap<String, Decl>,

    /// List of relative paths to include in search path when doing lookup in
    /// this module.
    ///
    /// Assuming we want to lookup `average`, which is in `std`. The root module
    /// does not contain the `average`. So instead:
    /// - look for `average` in root module and find nothing,
    /// - follow redirects in root module,
    /// - because of redirect `std`, so we look for `average` in `std`,
    /// - there is `average` is `std`,
    /// - result of the lookup is FQ ident `std.average`.
    pub redirects: Vec<pl::Ident>,

    /// A declaration that has been shadowed (overwritten) by this module.
    pub shadowed: Option<Box<Decl>>,
}

/// A struct containing information about a single declaration
/// within a PRQL module.
#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone)]
pub struct Decl {
    // TODO: make this plain usize, it is populated at creation anyway
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared_at: Option<usize>,

    pub kind: DeclKind,

    /// Some declarations (like relation columns) have an order to them.
    /// 0 means that the order is irrelevant.
    #[serde(skip_serializing_if = "is_zero")]
    pub order: usize,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<pl::Annotation>,
}

/// Declaration kind.
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, EnumAsInner)]
pub enum DeclKind {
    /// A nested namespace
    Module(Module),

    /// A function parameter (usually the implicit `this` param)
    // TODO: make this type non-optional
    Variable(Option<Ty>),

    TupleField,

    /// Contains a default value to be created in parent namespace when NS_INFER is matched.
    Infer(InferTarget),

    /// A generic type argument.
    /// It contains the candidate for this generic type that has been inferred during
    /// type validation. If the candidate is, for example, an `int` this means that
    /// this generic must be `int` or one of the previous type check would have failed.
    /// If the candidate is, for example, tuple `{a = int, b = bool}`, this means that
    /// previous type checks require the tuple to have fields `a` and `b`. It might contain
    /// other fields as well.
    ///
    /// Span describes the node that proposed the candidate.
    GenericParam(Option<(Ty, Option<Span>)>),

    Expr(Box<pl::Expr>),

    Ty(Ty),

    QueryDef(pl::QueryDef),

    /// Equivalent to the declaration pointed to by the fully qualified ident
    Import(pr::Ident),

    /// A declaration that has not yet been resolved.
    /// Created during the first pass of the AST, must not be present in
    /// a fully resolved module structure.
    Unresolved(pl::StmtKind),
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum InferTarget {
    Table,
    TupleField,
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
pub enum TableExpr {}

#[derive(Clone, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub enum TableColumn {
    Wildcard,
    Single(Option<String>),
}

impl std::fmt::Debug for RootModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.module.fmt(f)
    }
}

impl std::fmt::Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ds = f.debug_struct("Module");

        if !self.redirects.is_empty() {
            let redirects = self.redirects.iter().map(|x| x.to_string()).collect_vec();
            ds.field("redirects", &redirects);
        }

        if self.names.len() < 15 {
            ds.field("names", &DebugNames(&self.names));
        } else {
            ds.field("names", &format!("... {} entries ...", self.names.len()));
        }
        if self.shadowed.is_some() {
            ds.field("shadowed", &"(hidden)");
        }
        ds.finish()
    }
}

struct DebugNames<'a>(&'a HashMap<String, Decl>);

impl<'a> std::fmt::Debug for DebugNames<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dm = f.debug_map();
        for (n, decl) in self.0.iter().sorted_by_key(|x| x.0) {
            dm.entry(n, decl);
        }
        dm.finish()
    }
}

impl Default for DeclKind {
    fn default() -> Self {
        DeclKind::Module(Module::default())
    }
}

// TODO: convert to Decl::new
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
            Self::Variable(Some(arg0)) => {
                write!(f, "Variable of type {}", write_ty(arg0))
            }
            Self::Variable(None) => {
                write!(f, "Variable of unknown type")
            }
            Self::TupleField => write!(f, "TupleField"),
            Self::Infer(arg0) => write!(f, "Infer {arg0:?}"),
            Self::Expr(arg0) => write!(f, "Expr: {}", write_pl(*arg0.clone())),
            Self::Ty(arg0) => write!(f, "Ty: {}", write_ty(arg0)),
            Self::GenericParam(_) => write!(f, "GenericParam"),
            Self::QueryDef(_) => write!(f, "QueryDef"),
            Self::Import(arg0) => write!(f, "Import {arg0}"),
            Self::Unresolved(_) => write!(f, "Unresolved"),
        }
    }
}

fn is_zero(x: &usize) -> bool {
    *x == 0
}
