use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use std::collections::HashMap;
use strum_macros::Display;

use crate::ast::*;
use crate::ast_fold::*;
use crate::error::Span;

pub struct SemanticAnalyzer {
    ast: Node,

    // All declarations, over those out of scope
    pub(super) declarations: Vec<(Declaration, Option<Span>)>,

    // Scope we obtain after analyzing all nodes in AST
    last: Scope,

    // Scope we would need if we were to execute a query.
    // This will contain references that were resolved into *
    spill: Scope,
}

#[derive(Debug, EnumAsInner, Display, Clone)]
#[allow(dead_code)]
pub enum Declaration {
    Variable(VarDec),
    Table(String),
    Function(FuncDef),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VarDec {
    // index of the columns in the table
    pub position: Option<usize>,
    // the Node whose expr is equivalent to this variable
    pub declaration: Box<Node>,
    // for aliased columns and functions without arguments
    pub name: Option<String>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        SemanticAnalyzer {
            ast: Item::Query(Query { nodes: Vec::new() }).into(),
            declarations: Vec::new(),
            last: Scope::new_open(),
            spill: Scope::default(),
        }
    }

    pub fn get_ast(&self) -> &Node {
        &self.ast
    }

    pub fn get_table_columns(&self) -> Vec<Option<String>> {
        self.last
            .table
            .iter()
            .filter_map(|id| self.declarations[*id].0.as_variable().cloned())
            .map(|c| c.name)
            .collect()
    }

    /// Runs semantic analysis on the query, using current state.
    /// Appends query to current query.
    pub fn append(&mut self, query: Query) -> Result<()> {
        let mut nodes = self.fold_nodes(query.nodes)?;

        if let Some(ast) = self.ast.item.as_query_mut() {
            ast.nodes.append(&mut nodes);
        }
        Ok(())
    }

    fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.push((dec, span));
        self.declarations.len() - 1
    }

    fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();
        let is_variable = func_def.named_params.is_empty() && func_def.positional_params.is_empty();

        let span = Some(func_def.body.span);
        let id = self.declare(Declaration::Function(func_def), span);

        if is_variable {
            self.last
                .declare_variable(Some(&format!("$.{name}")), id, false);
        } else {
            self.last.functions.insert(name, id);
        }

        id
    }

    fn declare_variable(&mut self, var_dec: VarDec, span: Span) -> usize {
        let name = var_dec.name.clone();

        let id = self.declare(Declaration::Variable(var_dec), Some(span));

        self.last.declare_variable(name.as_deref(), id, true);

        id
    }

    fn lookup_variable(&mut self, ident: &str) -> Option<usize> {
        let id = self.last.lookup_variable(ident);
        if id.is_none() {
            self.spill.declare_variable(Some(ident), 0, false);
        }
        id
    }

    fn declare_table_column(&mut self, position: usize, node: &Node) {
        let position = Some(position);
        let var_dec = if let Some(named_expr) = node.item.as_named_expr() {
            VarDec {
                position,
                declaration: named_expr.expr.clone(),
                name: Some(named_expr.name.clone()),
            }
        } else {
            VarDec {
                position,
                declaration: Box::from(node.clone()),
                name: None,
            }
        };
        self.declare_variable(var_dec, node.span);
    }
}

impl AstFold for SemanticAnalyzer {
    // save functions declarations
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        items
            .into_iter()
            .map(|item| {
                Ok(match item {
                    Node {
                        item: Item::FuncDef(func_def),
                        ..
                    } => {
                        let func_def = fold_func_def(self, func_def)?;

                        self.declare_func(func_def);

                        None
                    }
                    _ => Some(self.fold_node(item)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }

    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        node.item = match node.item {
            Item::FuncCall(func_call) => {
                node.declared_at = self.last.functions.get(&func_call.name).cloned();

                Item::FuncCall(self.fold_func_call(func_call)?)
            }

            Item::Ident(ident) => {
                node.declared_at = self.lookup_variable(&ident);

                Item::Ident(self.fold_ident(ident)?)
            }

            // Item::InlinePipeline(p) => self.inline_pipeline(p)?,

            // Item::Ident(ident) => {
            //     if let Some(def) = self.functions_no_args.get(ident.as_str()) {
            //         def.body.item.clone()
            //     } else {
            //         Item::Ident(ident)
            //     }
            // }
            item => fold_item(self, item)?,
        };
        Ok(node)
    }

    fn fold_pipeline(&mut self, pipeline: Vec<Transformation>) -> Result<Vec<Transformation>> {
        pipeline
            .into_iter()
            .filter_map(|t| {
                if let Transformation::Select(_) = t {
                    self.last.clear_table();
                }
                let trans_name = t.name();

                match t {
                    Transformation::Select(nodes) | Transformation::Derive(nodes) => {
                        for (position, node) in nodes.into_iter().enumerate() {
                            let node = self.fold_node(node).unwrap();

                            self.declare_table_column(position, &node);
                        }
                        println!("{trans_name}: {:?}", self.get_table_columns());
                        None
                    }
                    t => Some(fold_transformation(self, t)),
                }
            })
            .collect()
    }
}

/// Scope within which we can reference variables, functions and tables
/// Provides fast lookups for different names.
#[derive(Debug, Default)]
pub struct Scope {
    // current table (result of last pipeline)
    table: Vec<usize>,

    // For each namespace (table), a list of its variables (columns)
    // "" is default namespace
    // "%" is namespace of functions without parameters
    namespaces: HashMap<String, HashMap<String, usize>>,

    // Functions with parameters (name is duplicated, but that's not much overhead)
    functions: HashMap<String, usize>,
}

impl Scope {
    /// Constructs new scope, which is "open". This means that it will resolve all
    /// variables and namespaces and not throw any errors.
    /// To be used without information of the database schema.
    fn new_open() -> Self {
        Scope {
            namespaces: HashMap::from([("".to_string(), HashMap::new())]),

            // and not anything else
            ..Default::default()
        }
    }

    /// Constructs new scope, which is "closed". This means that it will throw errors
    /// when resolving unknown variables or namespaces.
    #[allow(dead_code)]
    fn new() -> Self {
        Scope {
            ..Default::default()
        }
    }

    fn clear_table(&mut self) {
        self.table.clear();
    }

    fn declare_variable(&mut self, name: Option<&str>, id: usize, in_table: bool) {
        let mut overridden = None;

        if let Some(name) = name {
            let (namespace, variable) = name.rsplit_once('.').unwrap_or(("", name));

            let default = self.namespaces.entry("".to_string()).or_default();
            overridden = default.insert(variable.to_string(), id);

            if namespace.is_empty() {
                let namespace = self.namespaces.entry(namespace.to_string()).or_default();
                namespace.insert(variable.to_string(), id);
            }
        }

        if in_table {
            if let Some(overridden) = overridden {
                self.table.retain(|id| *id != overridden);
            }
            self.table.push(id);
        }
    }

    fn lookup_variable(&mut self, ident: &str) -> Option<usize> {
        let (namespace, variable) = ident.rsplit_once('.').unwrap_or(("", ident));

        if let Some(ns) = self.namespaces.get(namespace) {
            if let Some(decl_id) = ns.get(variable) {
                return Some(*decl_id);
            }
        }
        None
    }
}
