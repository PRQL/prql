use anyhow::Result;

use crate::ast::pl::*;
use crate::error::{Error, Reason, WithErrorInfo};

use super::Context;

/// Takes a resolved [Expr] and evaluates it a type expression that can be used to construct a type.
pub fn coerce_to_type(expr: Expr, context: &Context) -> Result<Ty, Error> {
    let (name, ty_expr) = coerce_to_named_type(expr, context)?;
    let kind = TyKind::TypeExpr(ty_expr);
    Ok(Ty { name, kind })
}

fn coerce_to_named_type(
    expr: Expr,
    context: &Context,
) -> Result<(Option<String>, TypeExpr), Error> {
    let name = expr.alias;
    let expr = coerce_kind_to_set(expr.kind, context).map_err(|e| e.with_span(expr.span))?;

    Ok((name, expr))
}

fn coerce_kind_to_set(expr: ExprKind, context: &Context) -> Result<TypeExpr, Error> {
    // primitives
    if let ExprKind::Type(set_expr) = expr {
        return Ok(set_expr);
    }

    // singletons
    if let ExprKind::Literal(lit) = expr {
        return Ok(TypeExpr::Singleton(lit));
    }

    // tuples
    if let ExprKind::List(elements) = expr {
        let mut set_elements = Vec::with_capacity(elements.len());

        for e in elements {
            let (name, ty) = coerce_to_named_type(e, context)?;

            set_elements.push(TupleElement::Single(name, ty));
        }

        return Ok(TypeExpr::Tuple(set_elements));
    }

    // arrays
    if let ExprKind::Array(elements) = expr {
        if elements.len() != 1 {
            return Err(Error::new_simple(
                "For type expressions, arrays must contain exactly one element.",
            ));
        }
        let items_type = elements.into_iter().next().unwrap();
        let (_, items_type) = coerce_to_named_type(items_type, context)?;

        return Ok(TypeExpr::Array(Box::new(items_type)));
    }

    // unions
    if let ExprKind::Binary {
        left,
        op: BinOp::Or,
        right,
    } = expr
    {
        let left = coerce_to_named_type(*left, context)?;
        let right = coerce_to_named_type(*right, context)?;

        // flatten nested unions
        let mut options = Vec::with_capacity(2);
        if let TypeExpr::Union(parts) = left.1 {
            options.extend(parts);
        } else {
            options.push(left);
        }
        if let TypeExpr::Union(parts) = right.1 {
            options.extend(parts);
        } else {
            options.push(right);
        }

        return Ok(TypeExpr::Union(options));
    }

    Err(Error::new_simple(format!(
        "not a type expression: {}",
        Expr::from(expr)
    )))
}

pub fn infer_type(node: &Expr, context: &Context) -> Result<Option<Ty>> {
    if let Some(ty) = &node.ty {
        return Ok(Some(ty.clone()));
    }

    let kind = match &node.kind {
        ExprKind::Literal(ref literal) => match literal {
            Literal::Null => return Ok(None),
            Literal::Integer(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Int)),
            Literal::Float(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Float)),
            Literal::Boolean(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Bool)),
            Literal::String(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Text)),
            Literal::Date(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Date)),
            Literal::Time(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Time)),
            Literal::Timestamp(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Timestamp)),
            Literal::ValueAndUnit(_) => return Ok(None), // TODO
            Literal::Relation(_) => unreachable!(),
        },

        ExprKind::Ident(_) | ExprKind::Pipeline(_) | ExprKind::FuncCall(_) => return Ok(None),

        ExprKind::SString(_) => return Ok(None),
        ExprKind::FString(_) => TyKind::TypeExpr(TypeExpr::Primitive(TyLit::Text)),
        ExprKind::Range(_) => return Ok(None), // TODO

        ExprKind::TransformCall(call) => TyKind::Table(call.infer_type(context)?),
        ExprKind::List(_) => return Ok(None), // TODO

        _ => return Ok(None),
    };
    Ok(Some(Ty { kind, name: None }))
}

#[allow(dead_code)]
fn too_many_arguments(call: &FuncCall, expected_len: usize, passed_len: usize) -> Error {
    let err = Error::new(Reason::Expected {
        who: Some(format!("{}", call.name)),
        expected: format!("{} arguments", expected_len),
        found: format!("{}", passed_len),
    });
    if passed_len >= 2 {
        err.with_help(format!(
            "If you are calling a function, you may want to add parentheses `{} [{:?} {:?}]`",
            call.name, call.args[0], call.args[1]
        ))
    } else {
        err
    }
}

impl Context {
    /// Validates that found node has expected type. Returns assumed type of the node.
    pub fn validate_type<F>(
        &mut self,
        found: &Expr,
        expected: &Option<Ty>,
        who: F,
    ) -> Result<Option<Ty>, Error>
    where
        F: FnOnce() -> Option<String>,
    {
        let found_ty = found.ty.clone();

        // infer
        let Some(expected) = expected else {
            return Ok(found_ty);
        };

        let Some(found_ty) = found_ty else {
            return Ok(if !expected.is_table() {
                // base case: infer expected type
                Some(expected.clone())
            } else {
                // special case: infer a table type
                // inferred tables are needed for s-strings that represent tables
                // similarly as normal table references, we want to be able to infer columns
                // of this table, which means it needs to be defined somewhere in the module structure.
                let frame =
                    self.declare_table_for_literal(found.id.unwrap(), None, found.alias.clone());

                // override the empty frame with frame of the new table
                Some(Ty {
                    kind: TyKind::Table(frame),
                    name: None,
                })
            });
        };

        let expected_is_above = expected.is_superset_of(&found_ty);
        if !expected_is_above {
            let e = Err(Error::new(Reason::Expected {
                who: who(),
                expected: format!("type `{}`", expected),
                found: format!("type `{}`", found_ty),
            })
            .with_span(found.span));
            if found_ty.is_function() && !expected.is_function() {
                let func_name = found.kind.as_closure().and_then(|c| c.name.as_ref());
                let to_what = func_name
                    .map(|n| format!("to function {n}"))
                    .unwrap_or_else(|| "in this function call?".to_string());

                return e.with_help(format!("Have you forgotten an argument {to_what}?"));
            };
            return e;
        }
        Ok(Some(found_ty))
    }
}

pub fn type_of_closure(closure: &Closure) -> TyFunc {
    TyFunc {
        args: closure.params.iter().map(|a| a.ty.clone()).collect(),
        return_ty: Box::new(closure.body_ty.clone()),
    }
}
