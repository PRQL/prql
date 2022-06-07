use anyhow::Result;

use crate::ast::ast_fold::*;
use crate::ast::*;

use super::complexity::determine_complexity;

use super::Context;

/// Runs type analysis on a query, using context.
///
/// Will determine type for each function call, variable or literal.
/// Will throw error on incorrect function argument type.
pub fn resolve_types(nodes: Vec<Node>, context: Context) -> Result<(Vec<Node>, Context)> {
    let mut resolver = TypeResolver::new(context);

    let nodes = resolver.fold_nodes(nodes)?;

    let nodes = determine_complexity(nodes, &resolver.context);

    Ok((nodes, resolver.context))
}

pub struct TypeResolver {
    pub context: Context,
}

impl TypeResolver {
    fn new(context: Context) -> Self {
        TypeResolver { context }
    }
}

impl TypeResolver {}

impl AstFold for TypeResolver {
    fn fold_node(&mut self, mut node: Node) -> Result<Node> {
        let ty = match &node.item {
            Item::Literal(literal) => match literal {
                Literal::Null => Ty::Infer,
                Literal::Integer(_) => TyLit::Integer.into(),
                Literal::Float(_) => TyLit::Float.into(),
                Literal::Boolean(_) => TyLit::Boolean.into(),
                Literal::String(_) => TyLit::String.into(),
                Literal::Date(_) => TyLit::Date.into(),
                Literal::Time(_) => TyLit::Time.into(),
                Literal::Timestamp(_) => TyLit::Timestamp.into(),
            },

            Item::Ident(_) => {
                let id = node.declared_at.unwrap();

                let mut expr = *self.context.declarations.take_expr(id)?;
                if matches!(expr.ty, Ty::Infer) {
                    expr = self.fold_node(expr)?;
                }
                let ty = expr.ty.clone();
                self.context.declarations.replace_expr(id, expr);

                ty
            }
            Item::Assign(ne) | Item::NamedArg(ne) => ne.expr.ty.clone(),
            Item::Pipeline(_) => todo!(),
            Item::Expr(_) => todo!(),
            Item::FuncCall(_) => todo!(),

            Item::SString(_) => Ty::Infer,
            Item::FString(_) => TyLit::String.into(),
            Item::Interval(_) => todo!(),
            Item::Range(_) => todo!(),
            _ => Ty::Infer,
        };

        node.ty = ty;

        Ok(node)
    }
}
