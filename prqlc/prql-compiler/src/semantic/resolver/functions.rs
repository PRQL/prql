use std::collections::HashMap;
use std::iter::zip;

use anyhow::Result;
use itertools::{Itertools, Position};

use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::*;
use prqlc_ast::{Ty, TyFunc, TyKind};

use crate::semantic::resolver::{transforms, types};
use crate::semantic::{NS_PARAM, NS_THAT, NS_THIS};
use crate::{Error, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn fold_function(&mut self, closure: Func, span: Option<Span>) -> Result<Expr> {
        let closure = self.resolve_function_types(closure)?;

        let closure = self.resolve_function_body(closure)?;

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
        log::debug!(
            "resolving args of function {}",
            closure
                .name_hint
                .clone()
                .unwrap_or_else(|| Ident::from_name("<unnamed>"))
        );
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
                let expr = transforms::resolve_special_func(self, closure)?;
                self.fold_expr(expr)?
            }
        } else {
            // base case: inline function
            let body = inline_function(closure);

            // TODO: do we still need this?
            //       when the tests pass again, we should try to remove and see what breaks.
            if let ExprKind::Func(mut inner_closure) = body.kind {
                // body couldn't been resolved - construct a closure to be evaluated later

                // inner_closure.env = func_env.into_exprs();
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

        Ok(Expr { span, ..res })
    }

    pub fn resolve_function_types(&mut self, mut func: Func) -> Result<Func> {
        func.params = func
            .params
            .into_iter()
            .map(|p| -> Result<_> {
                Ok(FuncParam {
                    ty: fold_type_opt(self, p.ty)?,
                    ..p
                })
            })
            .try_collect()?;
        func.return_ty = fold_type_opt(self, func.return_ty)?;
        Ok(func)
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
                    let lineage = arg.lineage.as_ref().unwrap();
                    if is_last {
                        self.root_mod.module.insert_frame(lineage, NS_THIS);
                    } else {
                        self.root_mod.module.insert_frame(lineage, NS_THAT);
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
        if let Ok(e) = &res {
            assert!(e.id.is_some(), "not resolved: {e:?}");
        }
        res
    }

    fn resolve_function_body(&mut self, mut func: Func) -> Result<Func> {
        if matches!(func.body.kind, ExprKind::Internal(_)) {
            return Ok(func);
        }

        self.root_mod.module.shadow(NS_PARAM);

        // insert type definitions of the params
        let module = self.root_mod.module.names.get_mut(NS_PARAM).unwrap();
        let module = module.kind.as_module_mut().unwrap();
        for param in func.params.iter().chain(func.named_params.iter()) {
            let ty = (param.ty).clone().unwrap_or_else(|| Ty::new(TyKind::Any));
            // TODO: continue here: anytype is incorrect here, it should be the generic type the is then restricted

            // infer types of params during resolution of the body
            // ty.infer = true;

            module.names.insert(
                param.name.clone(),
                Decl::from(DeclKind::Param(Box::new((ty, None)))),
            );

            // this should not be done in most cases, but only when this is an implicit closure
            // module.redirects.insert(Ident::from_name(&param.name));
        }

        let body = self.fold_expr(*func.body);

        // retrieve type definitions of the params
        //  let module = self.root_mod.module.names.get_mut(NS_PARAM).unwrap();
        //  let module = module.kind.as_module_mut().unwrap();
        //  for param in func.params.iter_mut().chain(func.named_params.iter_mut()) {
        //      let Some(decl) = module.names.remove(&param.name) else {
        //          continue;
        //      };

        //      let DeclKind::Param(ty) = decl.kind else {
        //          continue;
        //      };

        //      param.ty = Some(TyOrExpr::Ty(*ty));
        //  }

        self.root_mod.module.unshadow(NS_PARAM);

        let body = body?;

        // validate return type
        //  if let Some(body_ty) = &mut body.ty {
        //      let expected = func.return_ty.as_ref().and_then(|x| x.as_ty());
        //      let who = || {
        //          if let Some(name_hint) = &func.name_hint {
        //              Some(format!("return type of function {name_hint}"))
        //          } else {
        //              Some("return type".to_string())
        //          }
        //      };
        //      self.validate_type(body_ty, expected, &who)?;

        //      func.return_ty = Some(TyOrExpr::Ty(body_ty.clone()));
        //  }

        func.body = Box::new(body);
        Ok(func)
    }
}

fn extract_partial_application(mut func: Func, position: usize) -> Func {
    // Input:
    // Func {
    //     params: [x, y, z],
    //     args: [
    //         x,
    //         Func { # <--- this is arg_func below
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
    //     params: [substitute],
    //     args: [],
    //     body: Func {
    //         params: [x, y, z],
    //         args: [
    //             x,
    //             Func { # <--- this is arg_func below
    //                 params: [a, b],
    //                 args: [a, substitute],
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

    let substitute = format!("_partial_{}", arg.id.unwrap());
    let substitute_arg = Expr::new(Ident::from_path(vec![
        NS_PARAM.to_string(),
        substitute.clone(),
    ]));
    arg_func.args.push(substitute_arg);

    // set the arg func body to the parent func
    Func {
        name_hint: None,
        return_ty: None,
        body: Box::new(Expr::new(func)),
        params: vec![FuncParam {
            name: substitute,
            ty: None,
            default_value: None,
        }],
        named_params: Default::default(),
        args: Default::default(),
        env: Default::default(),
    }
}

fn expr_of_func(func: Func, span: Option<Span>) -> Expr {
    let ty = TyFunc {
        args: func
            .params
            .iter()
            .skip(func.args.len())
            .map(|a| a.ty.clone())
            .collect(),
        return_ty: Box::new(func.return_ty.clone()),
        name_hint: func.name_hint.clone(),
    };

    Expr {
        ty: Some(Ty::new(ty)),
        span,
        ..Expr::new(ExprKind::Func(Box::new(func)))
    }
}

fn inline_function(func: Func) -> Expr {
    // construct params map
    let mut params = HashMap::new();
    for (param, arg) in zip(func.params, func.args) {
        let param_name = param.name.split('.').last().unwrap();
        params.insert(param_name.to_string(), arg);
    }

    let mut inliner = FunctionInliner { params };

    inliner.fold_expr(*func.body).unwrap()
}
struct FunctionInliner {
    params: HashMap<String, Expr>,
}

impl PlFold for FunctionInliner {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        // this is the main thing: inline params
        if let ExprKind::Ident(ident) = &expr.kind {
            if ident.starts_with_part(NS_PARAM) {
                let param = self.params.get(&ident.name).unwrap();
                return Ok(param.clone());
            }
        }

        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }

    fn fold_func(&mut self, func: Func) -> Result<Func> {
        // params cannot appear within functions, so we can skip folding all together
        Ok(func)
    }
}
