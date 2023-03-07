use std::cmp::Ordering;
use std::collections::HashSet;

use anyhow::Result;

use crate::ast::pl::*;
use crate::error::{Error, Reason, WithErrorInfo};

use super::Context;

/// Takes a resolved [Expr] and evaluates it a set expression that can be used to construct a type.
pub fn eval_as_set(expr: Expr, context: &Context) -> Result<SetExpr, Error> {
    eval_kind_as_set(expr.kind, context).map_err(|e| e.with_span(expr.span))
}

pub fn eval_kind_as_set(expr: ExprKind, context: &Context) -> Result<SetExpr, Error> {
    // primitives
    if let ExprKind::Ident(fq_ident) = &expr {
        if fq_ident.path == vec!["std"] {
            let ty_lit = match fq_ident.name.as_str() {
                "int" => Some(TyLit::Int),
                "float" => Some(TyLit::Float),
                "bool" => Some(TyLit::Bool),
                "text" => Some(TyLit::Text),
                "date" => Some(TyLit::Date),
                "time" => Some(TyLit::Time),
                "timestamp" => Some(TyLit::Timestamp),
                _ => None,
            };
            if let Some(ty_lit) = ty_lit {
                return Ok(SetExpr::Primitive(ty_lit));
            }
        }
    }

    // singletons
    if let ExprKind::Literal(lit) = expr {
        return Ok(SetExpr::Singleton(lit));
    }

    // tuples
    if let ExprKind::List(elements) = expr {
        let mut set_elements = Vec::with_capacity(elements.len());

        for e in elements {
            set_elements.push(eval_tuple_element(e, context)?);
        }

        return Ok(SetExpr::Tuple(set_elements));
    }

    // unions
    if let ExprKind::Binary {
        left,
        op: BinOp::Or,
        right,
    } = expr
    {
        let left = eval_tuple_element(*left, context)?;
        let right = eval_tuple_element(*right, context)?;

        let mut options = Vec::with_capacity(2);
        options.push(left);
        options.push(right);

        return Ok(SetExpr::Union(options));
    }

    Err(Error::new_simple("not a set expression"))
}

pub fn eval_tuple_element(expr: Expr, context: &Context) -> Result<TupleElement, Error> {
    let name = expr.alias;
    let expr = eval_kind_as_set(expr.kind, context).map_err(|e| e.with_span(expr.span))?;

    Ok(TupleElement { name, expr })
}

pub fn infer_type(node: &Expr, context: &Context) -> Result<Ty> {
    if let Some(ty) = &node.ty {
        return Ok(ty.clone());
    }

    Ok(match &node.kind {
        ExprKind::Literal(ref literal) => match literal {
            Literal::Null => Ty::Infer,
            Literal::Integer(_) => TyLit::Int.into(),
            Literal::Float(_) => TyLit::Float.into(),
            Literal::Boolean(_) => TyLit::Bool.into(),
            Literal::String(_) => TyLit::Text.into(),
            Literal::Date(_) => TyLit::Date.into(),
            Literal::Time(_) => TyLit::Time.into(),
            Literal::Timestamp(_) => TyLit::Timestamp.into(),
            Literal::ValueAndUnit(_) => Ty::Infer, // TODO
            Literal::Relation(_) => unreachable!(),
        },

        ExprKind::Ident(_) | ExprKind::Pipeline(_) | ExprKind::FuncCall(_) => Ty::Infer,

        ExprKind::SString(_) => Ty::Infer,
        ExprKind::FString(_) => TyLit::Text.into(),
        ExprKind::Range(_) => Ty::Infer, // TODO

        ExprKind::TransformCall(call) => Ty::Table(call.infer_type(context)?),
        ExprKind::List(_) => Ty::Literal(TyLit::List),

        _ => Ty::Infer,
    })
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

/// Validates that found node has expected type. Returns assumed type of the node.
pub fn validate_type<F>(found: &Expr, expected: &Ty, who: F) -> Result<Ty, Error>
where
    F: FnOnce() -> Option<String>,
{
    let found_ty = found.ty.clone().unwrap();

    // infer
    if let Ty::Infer = expected {
        return Ok(found_ty);
    }
    if let Ty::Infer = found_ty {
        return Ok(if let Ty::Table(_) = expected {
            // inferred tables are needed for table s-strings
            // override the empty frame with frame of a table literal

            let input_name = (found.alias)
                .clone()
                .unwrap_or_else(|| format!("_literal_{}", found.id.unwrap()));

            Ty::Table(Frame {
                inputs: vec![FrameInput {
                    id: found.id.unwrap(),
                    name: input_name.clone(),
                    table: None,
                }],
                columns: vec![FrameColumn::All {
                    input_name,
                    except: HashSet::new(),
                }],
                ..Default::default()
            })
        } else {
            expected.clone()
        });
    }

    let expected_is_above = matches!(
        expected.partial_cmp(&found_ty),
        Some(Ordering::Equal | Ordering::Greater)
    );
    if !expected_is_above {
        let e = Err(Error::new(Reason::Expected {
            who: who(),
            expected: format!("type `{}`", expected),
            found: format!("type `{}`", found_ty),
        })
        .with_span(found.span));
        if matches!(found_ty, Ty::Function(_)) && !matches!(expected, Ty::Function(_)) {
            let func_name = found.kind.as_closure().and_then(|c| c.name.as_ref());
            let to_what = func_name
                .map(|n| format!("to function {n}"))
                .unwrap_or_else(|| "in this function call?".to_string());

            return e.with_help(format!("Have you forgotten an argument {to_what}?"));
        };
        return e;
    }
    Ok(found_ty)
}

pub fn type_of_closure(closure: &Closure) -> TyFunc {
    TyFunc {
        args: closure
            .params
            .iter()
            .map(|a| a.ty.clone().unwrap_or(Ty::Infer))
            .collect(),
        return_ty: Box::new(closure.body_ty.clone().unwrap_or(Ty::Infer)),
    }
}
