use std::collections::HashMap;
use std::iter::zip;

use anyhow::Result;
use itertools::{Itertools, Position};

use crate::ir::decl::{Decl, DeclKind, Module};
use crate::ir::pl::*;
use prqlc_ast::{Ty, TyFunc};

use crate::semantic::resolver::types;
use crate::semantic::{NS_PARAM, NS_THAT, NS_THIS};
use crate::{Error, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn fold_function(&mut self, closure: Func, span: Option<Span>) -> Result<Expr> {
        let closure = self.fold_function_types(closure)?;

        log::debug!(
            "func {} {}/{} params",
            closure.as_debug_name(),
            closure.args.len(),
            closure.params.len()
        );

        if closure.args.len() > closure.params.len() {
            return Err(Error::new_simple(format!(
                "Too many arguments to function `{}`",
                closure.as_debug_name()
            ))
            .with_span(span)
            .into());
        }

        let enough_args = closure.args.len() == closure.params.len();
        if !enough_args {
            return Ok(expr_of_func(closure, span));
        }

        // make sure named args are pushed into params
        let closure = if !closure.named_params.is_empty() {
            self.apply_args_to_closure(closure, [].into(), [].into())?
        } else {
            closure
        };

        // push the env
        let closure_env = Module::from_exprs(closure.env);
        self.root_mod.module.stack_push(NS_PARAM, closure_env);
        let closure = Func {
            env: HashMap::new(),
            ..closure
        };

        if log::log_enabled!(log::Level::Debug) {
            let name = closure
                .name_hint
                .clone()
                .unwrap_or_else(|| Ident::from_name("<unnamed>"));
            log::debug!("resolving args of function {}", name);
        }
        let res = self.resolve_function_args(closure)?;

        let closure = match res {
            Ok(func) => func,
            Err(func) => {
                return Ok(expr_of_func(func, span));
            }
        };

        let needs_window = (closure.params.last())
            .and_then(|p| p.ty.as_ref())
            .map(types::is_sub_type_of_array)
            .unwrap_or_default();

        // evaluate
        let res = if let ExprKind::Internal(operator_name) = &closure.body.kind {
            // special case: functions that have internal body

            if operator_name.starts_with("std.") {
                Expr {
                    ty: closure.return_ty,
                    needs_window,
                    ..Expr::new(ExprKind::RqOperator {
                        name: operator_name.clone(),
                        args: closure.args,
                    })
                }
            } else {
                let expr = self.resolve_special_func(closure, needs_window)?;
                self.fold_expr(expr)?
            }
        } else {
            // base case: materialize
            log::debug!("stack_push for {}", closure.as_debug_name());

            let (func_env, body) = env_of_closure(closure);

            self.root_mod.module.stack_push(NS_PARAM, func_env);

            // fold again, to resolve inner variables & functions
            let body = self.fold_expr(body)?;

            // remove param decls
            log::debug!("stack_pop: {:?}", body.id);
            let func_env = self.root_mod.module.stack_pop(NS_PARAM).unwrap();

            if let ExprKind::Func(mut inner_closure) = body.kind {
                // body couldn't been resolved - construct a closure to be evaluated later

                inner_closure.env = func_env.into_exprs();

                let (got, missing) = inner_closure.params.split_at(inner_closure.args.len());
                let missing = missing.to_vec();
                inner_closure.params = got.to_vec();

                Expr::new(ExprKind::Func(Box::new(Func {
                    name_hint: None,
                    args: vec![],
                    params: missing,
                    named_params: vec![],
                    body: Box::new(Expr::new(ExprKind::Func(inner_closure))),
                    return_ty: None,
                    env: HashMap::new(),
                })))
            } else {
                // resolved, return result
                body
            }
        };

        // pop the env
        self.root_mod.module.stack_pop(NS_PARAM).unwrap();

        Ok(Expr { span, ..res })
    }

    pub fn fold_function_types(&mut self, mut closure: Func) -> Result<Func> {
        closure.params = closure
            .params
            .into_iter()
            .map(|p| -> Result<_> {
                Ok(FuncParam {
                    ty: fold_type_opt(self, p.ty)?,
                    ..p
                })
            })
            .try_collect()?;
        closure.return_ty = fold_type_opt(self, closure.return_ty)?;
        Ok(closure)
    }

    pub fn apply_args_to_closure(
        &mut self,
        mut closure: Func,
        args: Vec<Expr>,
        mut named_args: HashMap<String, Expr>,
    ) -> Result<Func> {
        // named arguments are consumed only by the first function

        // named
        for mut param in closure.named_params.drain(..) {
            let param_name = param.name.split('.').last().unwrap_or(&param.name);
            let default = param.default_value.take().unwrap();

            let arg = named_args.remove(param_name).unwrap_or(*default);

            closure.args.push(arg);
            closure.params.insert(closure.args.len() - 1, param);
        }
        if let Some((name, _)) = named_args.into_iter().next() {
            // TODO: report all remaining named_args as separate errors
            anyhow::bail!(
                "unknown named argument `{name}` to closure {:?}",
                closure.name_hint
            )
        }

        // positional
        closure.args.extend(args);
        Ok(closure)
    }

    /// Resolves function arguments. Will return `Err(func)` is partial application is required.
    fn resolve_function_args(&mut self, to_resolve: Func) -> Result<Result<Func, Func>> {
        let mut closure = Func {
            args: vec![Expr::new(Literal::Null); to_resolve.args.len()],
            ..to_resolve
        };
        let mut partial_application_position = None;

        let func_name = &closure.name_hint;

        let (relations, other): (Vec<_>, Vec<_>) = zip(&closure.params, to_resolve.args)
            .enumerate()
            .partition(|(_, (param, _))| {
                let is_relation = param
                    .ty
                    .as_ref()
                    .map(|t| t.is_relation())
                    .unwrap_or_default();

                is_relation
            });

        let has_relations = !relations.is_empty();

        // resolve relational args
        if has_relations {
            self.root_mod.module.shadow(NS_THIS);
            self.root_mod.module.shadow(NS_THAT);

            for (pos, (index, (param, mut arg))) in relations.into_iter().with_position() {
                let is_last = matches!(pos, Position::Last | Position::Only);

                // just fold the argument alone
                if partial_application_position.is_none() {
                    arg = self
                        .fold_and_type_check(arg, param, func_name)?
                        .unwrap_or_else(|a| {
                            partial_application_position = Some(index);
                            a
                        });
                }
                log::debug!("resolved arg to {}", arg.kind.as_ref());

                // add relation frame into scope
                if partial_application_position.is_none() {
                    let frame = arg.lineage.as_ref().unwrap();
                    if is_last {
                        self.root_mod.module.insert_frame(frame, NS_THIS);
                    } else {
                        self.root_mod.module.insert_frame(frame, NS_THAT);
                    }
                }

                closure.args[index] = arg;
            }
        }

        // resolve other positional
        for (index, (param, mut arg)) in other {
            if partial_application_position.is_none() {
                if let ExprKind::Tuple(fields) = arg.kind {
                    // if this is a tuple, resolve elements separately,
                    // so they can be added to scope, before resolving subsequent elements.

                    let mut fields_new = Vec::with_capacity(fields.len());
                    for field in fields {
                        let field = self.fold_within_namespace(field, &param.name)?;

                        // add aliased columns into scope
                        if let Some(alias) = field.alias.clone() {
                            let id = field.id.unwrap();
                            self.root_mod.module.insert_frame_col(NS_THIS, alias, id);
                        }
                        fields_new.push(field);
                    }

                    // note that this tuple node has to be resolved itself
                    // (it's elements are already resolved and so their resolving
                    // should be skipped)
                    arg.kind = ExprKind::Tuple(fields_new);
                }

                arg = self
                    .fold_and_type_check(arg, param, func_name)?
                    .unwrap_or_else(|a| {
                        partial_application_position = Some(index);
                        a
                    });
            }

            closure.args[index] = arg;
        }

        if has_relations {
            self.root_mod.module.unshadow(NS_THIS);
            self.root_mod.module.unshadow(NS_THAT);
        }

        Ok(if let Some(position) = partial_application_position {
            log::debug!(
                "partial application of {} at arg {position}",
                closure.as_debug_name()
            );

            Err(extract_partial_application(closure, position))
        } else {
            Ok(closure)
        })
    }

    fn fold_and_type_check(
        &mut self,
        arg: Expr,
        param: &FuncParam,
        func_name: &Option<Ident>,
    ) -> Result<Result<Expr, Expr>> {
        let mut arg = self.fold_within_namespace(arg, &param.name)?;

        // don't validate types of unresolved exprs
        if arg.id.is_some() {
            // validate type

            let expects_func = param
                .ty
                .as_ref()
                .map(|t| t.is_function())
                .unwrap_or_default();
            if !expects_func && arg.kind.is_func() {
                return Ok(Err(arg));
            }

            let who = || {
                func_name
                    .as_ref()
                    .map(|n| format!("function {n}, param `{}`", param.name))
            };
            self.validate_expr_type(&mut arg, param.ty.as_ref(), &who)?;
        }

        Ok(Ok(arg))
    }

    fn fold_within_namespace(&mut self, expr: Expr, param_name: &str) -> Result<Expr> {
        let prev_namespace = self.default_namespace.take();

        if param_name.starts_with("noresolve.") {
            return Ok(expr);
        } else if let Some((ns, _)) = param_name.split_once('.') {
            self.default_namespace = Some(ns.to_string());
        } else {
            self.default_namespace = None;
        };

        let res = self.fold_expr(expr);
        self.default_namespace = prev_namespace;
        res
    }
}

fn extract_partial_application(mut func: Func, position: usize) -> Func {
    // Input:
    // Func {
    //     params: [x, y, z],
    //     args: [
    //         x,
    //         Func {
    //             params: [a, b],
    //             args: [a],
    //             body: arg_body
    //         },
    //         z
    //     ],
    //     body: parent_body
    // }

    // Output:
    // Func {
    //     params: [b],
    //     args: [],
    //     body: Func {
    //         params: [x, y, z],
    //         args: [
    //             x,
    //             Func {
    //                 params: [a, b],
    //                 args: [a, b],
    //                 body: arg_body
    //             },
    //             z
    //         ],
    //         body: parent_body
    //     }
    // }

    // This is quite in-efficient, especially for long pipelines.
    // Maybe it could be special-cased, for when the arg func has a single param.
    // In that case, it may be possible to pull the arg func up and basically swap
    // it with the parent func.

    let arg = func.args.get_mut(position).unwrap();
    let arg_func = arg.kind.as_func_mut().unwrap();

    let param_name = format!("_partial_{}", arg.id.unwrap());
    let substitute_arg = Expr::new(Ident::from_path(vec![
        NS_PARAM.to_string(),
        param_name.clone(),
    ]));
    arg_func.args.push(substitute_arg);

    // set the arg func body to the parent func
    Func {
        name_hint: None,
        return_ty: None,
        body: Box::new(Expr::new(func)),
        params: vec![FuncParam {
            name: param_name,
            ty: None,
            default_value: None,
        }],
        named_params: Default::default(),
        args: Default::default(),
        env: Default::default(),
    }
}

fn env_of_closure(closure: Func) -> (Module, Expr) {
    let mut func_env = Module::default();

    for (param, arg) in zip(closure.params, closure.args) {
        let v = Decl {
            declared_at: arg.id,
            kind: DeclKind::Expr(Box::new(arg)),
            ..Default::default()
        };
        let param_name = param.name.split('.').last().unwrap();
        func_env.names.insert(param_name.to_string(), v);
    }

    (func_env, *closure.body)
}

pub fn expr_of_func(func: Func, span: Option<Span>) -> Expr {
    let ty = TyFunc {
        args: func
            .params
            .iter()
            .skip(func.args.len())
            .map(|a| a.ty.clone())
            .collect(),
        return_ty: Box::new(func.return_ty.clone().or_else(|| func.body.ty.clone())),
        name_hint: func.name_hint.clone(),
    };

    Expr {
        ty: Some(Ty::new(ty)),
        span,
        ..Expr::new(ExprKind::Func(Box::new(func)))
    }
}
