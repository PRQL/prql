use std::cmp::Ordering;

use anyhow::Result;

use crate::ast::pl::*;
use crate::error::{Error, Reason, WithErrorInfo};

pub fn resolve_type(node: &Expr) -> Result<Ty> {
    if let Some(ty) = &node.ty {
        return Ok(ty.clone());
    }

    Ok(match &node.kind {
        ExprKind::Literal(ref literal) => match literal {
            Literal::Null => Ty::Infer,
            Literal::Integer(_) => TyLit::Integer.into(),
            Literal::Float(_) => TyLit::Float.into(),
            Literal::Boolean(_) => TyLit::Bool.into(),
            Literal::String(_) => TyLit::String.into(),
            Literal::Date(_) => TyLit::Date.into(),
            Literal::Time(_) => TyLit::Time.into(),
            Literal::Timestamp(_) => TyLit::Timestamp.into(),
            Literal::ValueAndUnit(_) => Ty::Infer, // TODO
        },

        ExprKind::Ident(_) | ExprKind::Pipeline(_) | ExprKind::FuncCall(_) => Ty::Infer,

        ExprKind::SString(_) => Ty::Infer,
        ExprKind::FString(_) => TyLit::String.into(),
        ExprKind::Range(_) => Ty::Infer, // TODO

        ExprKind::TransformCall(call) => Ty::Table(call.infer_type()?),
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
                columns: vec![FrameColumn::AllUnknown { input_name }],
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
            return e.with_help(match &found.kind {
                ExprKind::Closure(closure) => match &closure.name {
                    Some(name) => {
                        format!(
                            "Have you forgotten an argument to function `{}`?",
                            name.name
                        )
                    }
                    None => "Have you forgotten an argument in this function call?".to_string(),
                },
                _ => "Have you forgotten an argument in this function call?".to_string(),
            });
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
