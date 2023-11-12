use std::iter::zip;

use anyhow::Result;
use itertools::Itertools;
use prqlc_ast::{PrimitiveSet, TupleField, Ty, TyKind};

use crate::ir::pl::*;

use crate::semantic::write_pl;
use crate::{Error, Reason, WithErrorInfo};

use super::Resolver;

/// Takes a resolved [Expr] and evaluates it a type expression that can be used to construct a type.
// pub fn coerce_to_type(resolver: &mut Resolver, expr: Expr) -> Result<Ty> {
//     coerce_kind_to_set(resolver, expr.kind)
// }

// fn coerce_to_aliased_type(resolver: &mut Resolver, expr: Expr) -> Result<(Option<String>, Ty)> {
//     let name = expr.alias;
//     let expr = coerce_kind_to_set(resolver, expr.kind).map_err(|e| e.with_span(expr.span))?;

//     Ok((name, expr))
// }

// fn coerce_kind_to_set(resolver: &mut Resolver, expr: ExprKind) -> Result<Ty> {
//     Ok(match expr {
//         // already resolved type expressions (mostly primitives)
//         ExprKind::Type(set_expr) => set_expr,

//         // singletons
//         ExprKind::Literal(lit) => Ty::new(lit),

//         // tuples
//         ExprKind::Tuple(elements) => {
//             let mut set_elements = Vec::with_capacity(elements.len());

//             for e in elements {
//                 match try_restrict_range(e) {
//                     // special case: {x..}
//                     Ok(Range { start, .. }) => {
//                         let inner = match start {
//                             Some(x) => Some(coerce_to_type(resolver, *x)?),
//                             None => None,
//                         };

//                         set_elements.push(TupleField::Wildcard(inner))
//                     }

//                     // base: case
//                     Err(e) => {
//                         let (name, ty) = coerce_to_aliased_type(resolver, e)?;
//                         let ty = Some(ty);

//                         set_elements.push(TupleField::Single(name, ty));
//                     }
//                 }
//             }

//             Ty::new(TyKind::Tuple(set_elements))
//         }

//         // arrays
//         ExprKind::Array(elements) => {
//             if elements.len() != 1 {
//                 return Err(Error::new_simple(
//                     "For type expressions, arrays must contain exactly one element.",
//                 )
//                 .into());
//             }
//             let items_type = elements.into_iter().next().unwrap();
//             let (_, items_type) = coerce_to_aliased_type(resolver, items_type)?;

//             Ty::new(TyKind::Array(Box::new(items_type)))
//         }

//         // unions
//         ExprKind::RqOperator { name, args } if name == "std.or" => {
//             let [left, right]: [_; 2] = args.try_into().unwrap();
//             let left = coerce_to_type(resolver, left)?;
//             let right = coerce_to_type(resolver, right)?;

//             // flatten nested unions
//             let mut options = Vec::with_capacity(2);
//             if let TyKind::Union(parts) = left.kind {
//                 options.extend(parts);
//             } else {
//                 options.push((left.name.clone(), left));
//             }
//             if let TyKind::Union(parts) = right.kind {
//                 options.extend(parts);
//             } else {
//                 options.push((right.name.clone(), right));
//             }

//             Ty::new(TyKind::Union(options))
//         }

//         // functions
//         ExprKind::Func(func) => Ty::new(TyFunc {
//             args: func
//                 .params
//                 .into_iter()
//                 .map(|p| p.ty.map(|t| t))
//                 .collect_vec(),
//             return_ty: Box::new(resolver.fold_type(Some(func.body))?),
//         }),

//         _ => {
//             return Err(Error::new_simple(format!(
//                 "not a type expression: {}",
//                 write_pl(Expr::new(expr))
//             ))
//             .into())
//         }
//     })
// }

pub fn infer_type(node: &Expr) -> Result<Option<Ty>> {
    if let Some(ty) = &node.ty {
        return Ok(Some(ty.clone()));
    }

    let kind = match &node.kind {
        ExprKind::Literal(ref literal) => match literal {
            Literal::Null => TyKind::Singleton(Literal::Null),
            Literal::Integer(_) => TyKind::Primitive(PrimitiveSet::Int),
            Literal::Float(_) => TyKind::Primitive(PrimitiveSet::Float),
            Literal::Boolean(_) => TyKind::Primitive(PrimitiveSet::Bool),
            Literal::String(_) => TyKind::Primitive(PrimitiveSet::Text),
            Literal::Date(_) => TyKind::Primitive(PrimitiveSet::Date),
            Literal::Time(_) => TyKind::Primitive(PrimitiveSet::Time),
            Literal::Timestamp(_) => TyKind::Primitive(PrimitiveSet::Timestamp),
            Literal::ValueAndUnit(_) => return Ok(None), // TODO
        },

        ExprKind::Ident(_) | ExprKind::FuncCall(_) => return Ok(None),

        ExprKind::SString(_) => return Ok(None),
        ExprKind::FString(_) => TyKind::Primitive(PrimitiveSet::Text),

        ExprKind::TransformCall(_) => return Ok(None), // TODO
        ExprKind::Tuple(fields) => TyKind::Tuple(
            fields
                .iter()
                .map(|x| -> Result<_> {
                    let ty = infer_type(x)?;

                    Ok(TupleField::Single(x.alias.clone(), ty))
                })
                .try_collect()?,
        ),
        ExprKind::Array(items) => {
            let mut intersection = None;
            for item in items {
                let item_ty = infer_type(item)?;

                if let Some(item_ty) = item_ty {
                    if let Some(intersection) = &intersection {
                        if intersection != &item_ty {
                            // TODO: compute type intersection instead
                            return Ok(None);
                        }
                    } else {
                        intersection = Some(item_ty);
                    }
                }
            }
            let Some(items_ty) = intersection else {
                // TODO: return Array(Infer) instead of Infer
                return Ok(None);
            };
            TyKind::Array(Box::new(items_ty))
        }

        _ => return Ok(None),
    };
    Ok(Some(Ty {
        kind,
        name: None,
        span: None,
    }))
}

impl Resolver<'_> {
    /// Validates that found node has expected type. Returns assumed type of the node.
    pub fn validate_type<F>(
        &mut self,
        found: &mut Expr,
        expected: Option<&Ty>,
        who: &F,
    ) -> Result<(), Error>
    where
        F: Fn() -> Option<String>,
    {
        let found_ty = found.ty.clone();

        // infer
        let Some(expected) = expected else {
            // expected is none: there is no validation to be done
            return Ok(());
        };

        let Some(found_ty) = found_ty else {
            // found is none: infer from expected

            if found.lineage.is_none() && expected.is_relation() {
                // special case: infer a table type
                // inferred tables are needed for s-strings that represent tables
                // similarly as normal table references, we want to be able to infer columns
                // of this table, which means it needs to be defined somewhere in the module structure.
                let frame =
                    self.declare_table_for_literal(found.id.unwrap(), None, found.alias.clone());

                // override the empty frame with frame of the new table
                found.lineage = Some(frame)
            }

            // base case: infer expected type
            found.ty = Some(expected.clone());

            return Ok(());
        };

        let expected_is_above = match &mut found.kind {
            // special case of container type: tuple
            ExprKind::Tuple(found_fields) => {
                if expected.kind.is_any() {
                    return Ok(());
                }

                let ok = self.validate_tuple_type(found_fields, expected, who)?;
                if ok {
                    return Ok(());
                }
                false
            }

            // base case: compare types
            _ => is_super_type_of(expected, &found_ty),
        };
        if !expected_is_above {
            fn display_ty(ty: &Ty) -> String {
                if ty.is_tuple() {
                    "a tuple".to_string()
                } else {
                    format!("type `{}`", crate::codegen::write_ty(ty))
                }
            }

            let who = who();
            let is_join = who
                .as_ref()
                .map(|x| x.contains("std.join"))
                .unwrap_or_default();

            let mut e = Err(Error::new(Reason::Expected {
                who,
                expected: display_ty(expected),
                found: display_ty(&found_ty),
            })
            .with_span(found.span));

            if found_ty.is_function() && !expected.is_function() {
                let func_name = found.kind.as_func().and_then(|c| c.name_hint.as_ref());
                let to_what = func_name
                    .map(|n| format!("to function {n}"))
                    .unwrap_or_else(|| "in this function call?".to_string());

                e = e.push_hint(format!("Have you forgotten an argument {to_what}?"));
            }

            if is_join && found_ty.is_tuple() && !expected.is_tuple() {
                e = e.push_hint("Try using `(...)` instead of `{...}`");
            }

            if let Some(expected_name) = &expected.name {
                let expanded = crate::codegen::write_ty_kind(&expected.kind);
                e = e.push_hint(format!("Type `{expected_name}` expands to `{expanded}`"));
            }

            return e;
        }
        Ok(())
    }

    fn validate_tuple_type<F>(
        &mut self,
        found_fields: &mut [Expr],
        expected: &Ty,
        who: &F,
    ) -> Result<bool, Error>
    where
        F: Fn() -> Option<String>,
    {
        let Some(expected_fields) = find_potential_tuple_fields(expected) else {
            return Ok(false);
        };

        let mut found = found_fields.iter_mut();

        for expected_field in expected_fields {
            match expected_field {
                TupleField::Single(_, expected_kind) => match found.next() {
                    Some(found_field) => {
                        self.validate_type(found_field, expected_kind.as_ref(), who)?
                    }
                    None => {
                        return Ok(false);
                    }
                },
                TupleField::Wildcard(expected_wildcard) => {
                    for found_field in found {
                        self.validate_type(found_field, expected_wildcard.as_ref(), who)?;
                    }
                    return Ok(true);
                }
            }
        }

        Ok(found.next().is_none())
    }
}

#[allow(dead_code)]
fn too_many_arguments(call: &FuncCall, expected_len: usize, passed_len: usize) -> Error {
    let err = Error::new(Reason::Expected {
        who: Some(write_pl(*call.name.clone())),
        expected: format!("{} arguments", expected_len),
        found: format!("{}", passed_len),
    });
    if passed_len >= 2 {
        err.push_hint(format!(
            "if you are calling a function, you may want to add parentheses `{} [{:?} {:?}]`",
            write_pl(*call.name.clone()),
            call.args[0],
            call.args[1]
        ))
    } else {
        err
    }
}

fn find_potential_tuple_fields(expected: &Ty) -> Option<&Vec<TupleField>> {
    match &expected.kind {
        TyKind::Tuple(fields) => Some(fields),
        TyKind::Union(variants) => {
            for (_, variant) in variants {
                if let Some(fields) = find_potential_tuple_fields(variant) {
                    return Some(fields);
                }
            }
            None
        }
        _ => None,
    }
}

/// Analogous to [crate::ir::pl::Lineage::rename()]
pub fn rename_relation(ty_kind: &mut TyKind, alias: String) {
    if let TyKind::Array(items_ty) = ty_kind {
        rename_tuples(&mut items_ty.kind, alias);
    }
}

fn rename_tuples(ty_kind: &mut TyKind, alias: String) {
    flatten_tuples(ty_kind);

    if let TyKind::Tuple(fields) = ty_kind {
        let inner_fields = std::mem::take(fields);

        let ty = Ty::new(TyKind::Tuple(inner_fields));
        fields.push(TupleField::Single(Some(alias), Some(ty)));
    }
}

fn flatten_tuples(ty_kind: &mut TyKind) {
    if let TyKind::Tuple(fields) = ty_kind {
        let mut new_fields = Vec::new();

        for field in fields.drain(..) {
            let TupleField::Single(name, Some(ty)) = field else {
                new_fields.push(field);
                continue;
            };

            // recurse
            // let ty = ty.flatten_tuples();

            let TyKind::Tuple(inner_fields) = ty.kind else {
                new_fields.push(TupleField::Single(name, Some(ty)));
                continue;
            };
            new_fields.extend(inner_fields);
        }

        fields.extend(new_fields);
    }
}

pub fn is_super_type_of(superset: &Ty, subset: &Ty) -> bool {
    if superset.is_relation() && subset.is_relation() {
        return true;
    }
    is_super_type_of_kind(&superset.kind, &subset.kind)
}

pub fn is_sub_type_of_array(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Array(_) => true,
        TyKind::Union(elements) => elements.iter().any(|(_, e)| is_sub_type_of_array(e)),
        _ => false,
    }
}

fn is_super_type_of_kind(superset: &TyKind, subset: &TyKind) -> bool {
    match (superset, subset) {
        (TyKind::Any, _) => true,
        (_, TyKind::Any) => false,
        (TyKind::Primitive(l0), TyKind::Primitive(r0)) => l0 == r0,

        (one, TyKind::Union(many)) => many
            .iter()
            .all(|(_, each)| is_super_type_of_kind(one, &each.kind)),

        (TyKind::Union(many), one) => many
            .iter()
            .any(|(_, any)| is_super_type_of_kind(&any.kind, one)),

        (TyKind::Function(None), TyKind::Function(_)) => true,
        (TyKind::Function(Some(_)), TyKind::Function(None)) => true,
        (TyKind::Function(Some(sup)), TyKind::Function(Some(sub))) => {
            if is_not_super_type_of(sup.return_ty.as_ref(), sub.return_ty.as_ref()) {
                return false;
            }
            if sup.args.len() != sub.args.len() {
                return false;
            }
            for (sup_arg, sub_arg) in zip(&sup.args, &sub.args) {
                if is_not_super_type_of(sup_arg, sub_arg) {
                    return false;
                }
            }

            true
        }

        (l, r) => l == r,
    }
}

fn is_not_super_type_of(sup: &Option<Ty>, sub: &Option<Ty>) -> bool {
    if let Some(sub_ret) = sub {
        if let Some(sup_ret) = sup {
            if !is_super_type_of(sup_ret, sub_ret) {
                return true;
            }
        }
    }
    false
}
