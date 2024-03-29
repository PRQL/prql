use std::collections::HashMap;
use std::iter::zip;

use crate::Result;
use itertools::Itertools;

use crate::ast::{Ty, TyFunc};
use crate::ir::decl::{Decl, DeclKind, Module};
use crate::ir::pl::*;
use crate::semantic::{NS_GENERIC, NS_LOCAL, NS_PARAM, NS_THAT, NS_THIS};
use crate::{Error, Span, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    pub fn fold_function(
        &mut self,
        closure: Box<Func>,
        span: Option<Span>,
    ) -> Result<Expr> {
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
            .with_span(span));
        }

        let enough_args = closure.args.len() == closure.params.len();
        if !enough_args {
            return Ok(*expr_of_func(closure, span));
        }

        // make sure named args are pushed into params
        let closure = if !closure.named_params.is_empty() {
            self.apply_args_to_closure(closure, [].into(), [].into())?
        } else {
            closure
        };

        // push the env
        let closure_env = Module::from_exprs(closure.env);
        self.root_mod.local_mut().stack_push(NS_PARAM, closure_env);
        let closure = Box::new(Func {
            env: HashMap::new(),
            ..*closure
        });

        log::debug!("resolving args of function {}", closure.as_debug_name());
        let res = self.resolve_function_args(closure)?;

        let mut closure = match res {
            Ok(func) => func,
            Err(func) => {
                return Ok(*expr_of_func(func, span));
            }
        };

        closure.return_ty = self
            .resolve_generic_args_opt(closure.return_ty)
            .with_span_fallback(span)?;

        let needs_window = (closure.params.last())
            .and_then(|p| p.ty.as_ref())
            .map(|t| t.kind.is_array())
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
            self.materialize_function(closure)?
        };

        // pop the env
        self.root_mod.local_mut().stack_pop(NS_PARAM).unwrap();

        Ok(Expr { span, ..res })
    }

    #[allow(clippy::boxed_local)]
    fn materialize_function(&mut self, closure: Box<Func>) -> Result<Expr> {
        log::debug!("stack_push for {}", closure.as_debug_name());

        let (func_env, body, return_ty) = env_of_closure(*closure);

        self.root_mod.local_mut().stack_push(NS_PARAM, func_env);

        // fold again, to resolve inner variables & functions
        let body = self.fold_expr(body)?;

        // remove param decls
        log::debug!("stack_pop: {:?}", body.id);
        let func_env = self.root_mod.local_mut().stack_pop(NS_PARAM).unwrap();

        Ok(if let ExprKind::Func(mut inner_closure) = body.kind {
            // body couldn't been resolved - construct a closure to be evaluated later

            inner_closure.env = func_env.into_exprs();

            let (got, missing) = inner_closure.params.split_at(inner_closure.args.len());
            let missing = missing.to_vec();
            inner_closure.params = got.to_vec();

            Expr::new(ExprKind::Func(Box::new(Func {
                name_hint: None,
                args: vec![],
                params: missing,
                body: Box::new(Expr::new(ExprKind::Func(inner_closure))),

                // these don't matter
                named_params: Default::default(),
                return_ty: Default::default(),
                env: Default::default(),
                generic_type_params: Default::default(),
                implicit_closure: None,
            })))
        } else {
            // resolved, return result

            // make sure to use the resolved type
            let mut body = body;
            if let Some(ret_ty) = *return_ty {
                body.ty = Some(ret_ty);
            }

            body
        })
    }

    /// Folds function types, so they are resolved to material types, ready for type checking.
    /// Requires id of the function call node, so it can be used to generic type arguments.
    pub fn fold_function_types(&mut self, mut func: Box<Func>) -> Result<Box<Func>> {
        // prepare generic arguments
        for generic_param in &func.generic_type_params {
            // TODO: fold bounds
            // let domain: Vec<Ty> = generic_param
            //     .bounds
            //     .iter()
            //     .map(|t| self.fold_type(t.clone()))
            //     .try_collect()?;

            // register the generic type param in the resolver
            let ident = Ident::from_path(vec![NS_GENERIC, generic_param.name.as_str()]);
            let decl = Decl::from(DeclKind::GenericParam(vec![]));
            self.root_mod.local_mut().insert(ident, decl).unwrap();
        }

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
        mut closure: Box<Func>,
        args: Vec<Expr>,
        mut named_args: HashMap<String, Expr>,
    ) -> Result<Box<Func>> {
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
            return Err(Error::new_simple(format!(
                "unknown named argument `{name}` to closure {:?}",
                closure.name_hint
            )));
        }

        // positional
        closure.args.extend(args);
        Ok(closure)
    }

    /// Resolves function arguments. Will return `Err(func)` is partial application is required.
    fn resolve_function_args(
        &mut self,
        #[allow(clippy::boxed_local)] to_resolve: Box<Func>,
    ) -> Result<Result<Box<Func>, Box<Func>>> {
        let mut func = Box::new(Func {
            args: vec![Expr::new(Literal::Null); to_resolve.args.len()],
            ..*to_resolve
        });
        let mut partial_application_position = None;

        let func_name = &func.name_hint;

        let mut param_args = zip(&func.params, to_resolve.args)
            .map(Box::new)
            .map(Some)
            .collect_vec();

        // pull out this and that
        let impl_cl_pos = func.implicit_closure.as_ref().map(|i| i.param as usize);
        let this_pos = func.implicit_closure.as_ref().and_then(|i| i.this);
        let that_pos = func.implicit_closure.as_ref().and_then(|i| i.that);

        // prepare order
        let order = this_pos
            .into_iter()
            .chain(that_pos)
            .map(|x| x as usize)
            .chain(0..param_args.len())
            .unique()
            .collect_vec();

        for index in order {
            let (param, mut arg) = *param_args[index].take().unwrap();

            if partial_application_position.is_none() {
                if impl_cl_pos.map_or(false, |pos| pos == index) {
                    if let Some(pos) = this_pos {
                        let ty = func.args[pos as usize].ty.as_ref().unwrap();
                        self.root_mod.local_mut().insert_frame(ty, NS_THIS);
                    }
                    if let Some(pos) = that_pos {
                        let ty = func.args[pos as usize].ty.as_ref().unwrap();
                        self.root_mod.local_mut().insert_frame(ty, NS_THAT);
                    }
                }

                arg = self
                    .fold_and_type_check(arg, param, func_name)?
                    .unwrap_or_else(|a| {
                        partial_application_position = Some(index);
                        a
                    });

                if impl_cl_pos.map_or(false, |pos| pos == index) {
                    if this_pos.is_some() {
                        self.root_mod.local_mut().unshadow(NS_THIS);
                    }
                    if that_pos.is_some() {
                        self.root_mod.local_mut().unshadow(NS_THAT);
                    }
                }
            }
            func.args[index] = arg;
        }

        Ok(if let Some(position) = partial_application_position {
            log::debug!(
                "partial application of {} at arg {position}",
                func.as_debug_name()
            );

            Err(extract_partial_application(func, position))
        } else {
            Ok(func)
        })
    }

    fn fold_and_type_check(
        &mut self,
        arg: Expr,
        param: &FuncParam,
        func_name: &Option<Ident>,
    ) -> Result<Result<Expr, Expr>> {
        // fold
        if param.name.starts_with("noresolve.") {
            return Ok(Ok(arg));
        };

        self.root_mod.local_mut().shadow(NS_GENERIC);
        let mut arg = self.fold_expr(arg)?;
        self.root_mod.local_mut().unshadow(NS_GENERIC);

        // special case: (I forgot why this is needed)
        let expects_func = param
            .ty
            .as_ref()
            .map(|t| t.kind.is_function())
            .unwrap_or_default();
        if !expects_func && arg.kind.is_func() {
            return Ok(Err(arg));
        }

        // validate type
        let who = || {
            func_name
                .as_ref()
                .map(|n| format!("function {n}, param `{}`", param.name))
        };
        self.validate_expr_type(&mut arg, param.ty.as_ref(), &who)?;
        Ok(Ok(arg))
    }
}

fn extract_partial_application(mut func: Box<Func>, position: usize) -> Box<Func> {
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
        NS_LOCAL.to_string(),
        NS_PARAM.to_string(),
        param_name.clone(),
    ]));
    arg_func.args.push(substitute_arg);

    // set the arg func body to the parent func
    Box::new(Func {
        name_hint: None,
        return_ty: None,
        body: Box::new(Expr::new(ExprKind::Func(func))),
        params: vec![FuncParam {
            name: param_name,
            ty: None,
            default_value: None,
        }],
        named_params: Default::default(),
        args: Default::default(),
        env: Default::default(),
        generic_type_params: Default::default(),
        implicit_closure: Default::default(),
    })
}

fn env_of_closure(closure: Func) -> (Module, Expr, Box<Option<Ty>>) {
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

    (func_env, *closure.body, Box::new(closure.return_ty))
}

pub fn expr_of_func(func: Box<Func>, span: Option<Span>) -> Box<Expr> {
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

    Box::new(Expr {
        ty: Some(Ty::new(ty)),
        span,
        ..Expr::new(ExprKind::Func(func))
    })
}
