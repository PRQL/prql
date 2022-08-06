use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::scope::NS_PARAM;
use super::{Declaration, Declarations, Scope};
use crate::ast::*;
use crate::error::Span;

/// Context of the pipeline.
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Context {
    /// Map of all accessible names (for each namespace)
    pub(crate) scope: Scope,

    /// All declarations, even those out of scope
    pub(crate) declarations: Declarations,
}

impl Context {
    pub fn declare(&mut self, dec: Declaration, span: Option<Span>) -> usize {
        self.declarations.0.push((dec, span));
        self.declarations.0.len() - 1
    }

    pub fn declare_func(&mut self, func_def: FuncDef) -> usize {
        let name = func_def.name.clone();

        let span = func_def.body.span;
        let id = self.declare(Declaration::Function(func_def), span);

        self.scope.add_function(name, id);

        id
    }

    pub fn declare_table(&mut self, t: &mut TableRef) {
        let name = t.alias.clone().unwrap_or_else(|| t.name.clone());
        let decl = Declaration::Table(name.clone());

        let table_id = self.declare(decl, None);
        t.declared_at = Some(table_id);

        let var_name = format!("{name}.*");
        self.scope.add(var_name, table_id);
    }

    pub fn declare_func_param(&mut self, node: &Node) -> usize {
        let name = match &node.item {
            Item::Ident(ident) => ident.clone(),
            Item::NamedArg(NamedExpr { name, .. }) => name.clone(),
            _ => unreachable!(),
        };

        // doesn't matter, will get overridden anyway
        let decl = Box::new(Item::Ident("".to_string()).into());

        let id = self.declare(Declaration::Expression(decl), None);

        self.scope.add(format!("{NS_PARAM}.{name}"), id);

        id
    }
}

impl From<Declaration> for anyhow::Error {
    fn from(dec: Declaration) -> Self {
        // panic!("Unexpected declaration type: {dec:?}");
        anyhow::anyhow!("Unexpected declaration type: {dec:?}")
    }
}
