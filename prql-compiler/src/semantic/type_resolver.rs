use std::cmp::Ordering;

use anyhow::Result;

use crate::ast::*;
use crate::error::{Error, Reason};

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

        ExprKind::SString(_) => Ty::Infer, // TODO
        ExprKind::FString(_) => TyLit::String.into(),
        ExprKind::Range(_) => Ty::Infer,    // TODO

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
        return Ok(expected.clone());
    }

    let expected_is_above = matches!(
        expected.partial_cmp(&found_ty),
        Some(Ordering::Equal | Ordering::Greater)
    );
    if !expected_is_above {
        return Err(Error::new(Reason::Expected {
            who: who(),
            expected: format!("type `{}`", expected),
            found: format!("type `{}`", found_ty),
        })
        .with_span(found.span));
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
        named: closure
            .named_params
            .iter()
            .map(|a| (a.name.clone(), a.ty.clone().unwrap_or(Ty::Infer)))
            .collect(),
        return_ty: Box::new(Ty::Infer), // TODO
    }
}
