use anyhow::Result;
use itertools::Itertools;

use crate::ast::*;
use crate::ast_fold::*;

use super::context::Context;
use super::context::TableColumn;

/// Runs semantic analysis on the query, using current state.
/// Appends query to current query.
///
/// Note that analyzer removes function declarations, derive and select
/// transformations from AST and saves them as current context.
pub fn resolve(nodes: Vec<Node>, context: Option<Context>) -> Result<(Vec<Node>, Context)> {
    let context = context.unwrap_or_default();

    let mut resolver = Resolver::new(context);

    let nodes = resolver.fold_nodes(nodes)?;

    Ok((nodes, resolver.context))
}

/// Can fold (walk) over AST and for each function calls or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,
}

impl Resolver {
    fn new(context: Context) -> Self {
        Resolver { context }
    }

    fn declare_table_columns(&mut self, nodes: Vec<Node>) -> Result<Vec<Node>> {
        nodes
            .into_iter()
            .enumerate()
            .map(|(position, node)| {
                let node = self.fold_node(node)?;

                self.context.declare_table_column(position, &node);
                Ok(node)
            })
            .try_collect()
    }
}

impl AstFold for Resolver {
    // save functions declarations
    fn fold_nodes(&mut self, items: Vec<Node>) -> Result<Vec<Node>> {
        // We cut out function def, so we need to run it
        // here rather than in `fold_func_def`.
        items
            .into_iter()
            .map(|item| {
                Ok(match item {
                    Node {
                        item: Item::FuncDef(mut func_def),
                        ..
                    } => {
                        // declare variables
                        for param in &mut func_def.named_params {
                            param.declared_at = Some(self.context.declare_func_param(param));
                        }
                        for param in &mut func_def.positional_params {
                            param.declared_at = Some(self.context.declare_func_param(param));
                        }

                        // fold body
                        func_def.body = Box::new(self.fold_node(*func_def.body)?);

                        // clear declared variables
                        self.context.clear_scope();

                        self.context.declare_func(func_def);
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
                node.declared_at = self.context.functions.get(&func_call.name).cloned();

                Item::FuncCall(self.fold_func_call(func_call)?)
            }

            Item::Ident(ident) => {
                node.declared_at = self.context.lookup_variable(&ident);

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

    fn fold_pipeline(&mut self, pipeline: Vec<Transform>) -> Result<Vec<Transform>> {
        pipeline
            .into_iter()
            .map(|t| {
                // let trans_name = t.name();

                Ok(match t {
                    Transform::From(_) => {
                        self.context.table.clear();
                        self.context.table.push(TableColumn::All);

                        Some(fold_transformation(self, t)?)
                    }

                    Transform::Select(nodes) => {
                        self.context.table.clear();

                        self.declare_table_columns(nodes)?;
                        None
                    }
                    Transform::Derive(nodes) => {
                        self.declare_table_columns(nodes)?;
                        None
                    }
                    Transform::Aggregate { by, select } => {
                        self.context.table.clear();

                        let by = self.declare_table_columns(by)?;
                        self.declare_table_columns(select)?;

                        Some(Transform::Aggregate { by, select: vec![] })
                    }
                    t => Some(fold_transformation(self, t)?),
                })
            })
            .filter_map(|x| x.transpose())
            .try_collect()
    }
}
