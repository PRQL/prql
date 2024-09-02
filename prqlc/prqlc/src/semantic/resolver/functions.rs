use itertools::Itertools;
use std::collections::HashMap;

use crate::codegen::write_ty;
use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::*;
use crate::pr::{GenericTypeParam, Ty, TyFunc, TyKind, TyTupleField};
use crate::semantic::{write_pl, NS_GENERIC, NS_LOCAL, NS_THAT, NS_THIS};
use crate::{Error, Result, Span, WithErrorInfo};

use super::scope::Scope;
use super::types::TypeReplacer;
use super::Resolver;

impl Resolver<'_> {
    /// Folds function types, so they are resolved to material types, ready for type checking.
    /// Requires id of the function call node, so it can be used to generic type arguments.
    pub fn resolve_func(&mut self, mut func: Box<Func>) -> Result<Box<Func>> {
        let mut scope = Scope::new();

        // prepare generic arguments
        for generic_param in &func.generic_type_params {
            let bound: Option<Ty> = generic_param
                .bound
                .clone()
                .map(|b| self.fold_type(b))
                .transpose()?;

            // register the generic type param in the resolver
            let generic = Decl::from(DeclKind::GenericParam(bound.map(|b| (b, None))));
            scope.types.insert(generic_param.name.clone(), generic);
        }
        self.scopes.push(scope);

        // fold types
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

        // put params into scope
        prepare_scope_of_func(self.scopes.last_mut().unwrap(), &func);

        func.body = Box::new(self.fold_expr(*func.body)?);

        // validate that the body has correct type
        self.validate_expr_type(&mut func.body, func.return_ty.as_ref(), &|| None)?;

        // pop the scope
        let mut scope = self.scopes.pop().unwrap();

        // pop generic types
        if !func.generic_type_params.is_empty() {
            let mut new_generic_type_params = Vec::new();
            let mut finalized_args = HashMap::new();
            for gtp in func.generic_type_params {
                let inferred_generic = scope.types.swap_remove(&gtp.name).unwrap();
                let inferred_type = inferred_generic.kind.into_generic_param().unwrap();

                match inferred_type {
                    Some((inferred_type, _)) if !inferred_type.kind.is_tuple() => {
                        // The bounds of this generic type param restrict it to a single type.
                        // In other words: we have enough information to conclude that this param can only be one specific type.
                        // So we can finalize it to that type and inline any references to the param.
                        log::debug!("finalizing generic param {}", gtp.name);

                        finalized_args.insert(
                            Ident::from_path(vec![NS_LOCAL, NS_GENERIC, &gtp.name]),
                            inferred_type,
                        );
                    }
                    _ => {
                        let bound = inferred_type.map(|(t, _)| t);
                        new_generic_type_params.push(GenericTypeParam {
                            name: gtp.name,
                            bound,
                        })
                    }
                }
            }
            func.generic_type_params = new_generic_type_params;

            func = Box::new(TypeReplacer::on_func(*func, finalized_args));
        }

        Ok(func)
    }

    pub fn apply_args_to_function(
        &mut self,
        func: Box<Expr>,
        args: Vec<Expr>,
        mut _named_args: HashMap<String, Expr>,
    ) -> Result<FuncApplication> {
        let mut fn_app = if let ExprKind::FuncApplication(fn_app) = func.kind {
            fn_app
        } else {
            FuncApplication {
                func,
                args: Vec::new(),
            }
        };

        // named
        // let fn_ty = fn_app.func.ty.as_ref().unwrap();
        // let fn_ty = fn_ty.kind.as_function().unwrap();
        // let fn_ty = fn_ty.as_ref().unwrap().clone();
        // for mut param in fn_ty.named_params.drain(..) {
        //     let param_name = param.name.split('.').last().unwrap_or(&param.name);
        //     let default = param.default_value.take().unwrap();
        //     let arg = named_args.remove(param_name).unwrap_or(*default);
        //     fn_app.args.push(arg);
        //     fn_app.func.params.insert(fn_app.args.len() - 1, param);
        // }
        // if let Some((name, _)) = named_args.into_iter().next() {
        //     // TODO: report all remaining named_args as separate errors
        //     return Err(Error::new_simple(format!(
        //         "unknown named argument `{name}` to closure {:?}",
        //         fn_app.func.name_hint
        //     )));
        // }

        // positional
        fn_app.args.extend(args);
        Ok(fn_app)
    }

    pub fn resolve_func_application(
        &mut self,
        fn_app: FuncApplication,
        span: Option<Span>,
    ) -> Result<Expr> {
        let metadata = self.gather_func_metadata(&fn_app.func);

        let fn_ty = fn_app.func.ty.as_ref().unwrap();
        let fn_ty = fn_ty.kind.as_function().unwrap();
        let fn_ty = fn_ty.as_ref().unwrap().clone();

        log::debug!(
            "func {} {}/{} params",
            metadata.as_debug_name(),
            fn_app.args.len(),
            fn_ty.params.len()
        );

        if fn_app.args.len() > fn_ty.params.len() {
            return Err(Error::new_simple(format!(
                "Too many arguments to function `{}`",
                metadata.as_debug_name()
            ))
            .with_span(span));
        }

        let enough_args = fn_app.args.len() == fn_ty.params.len();
        if !enough_args {
            return Ok(*expr_of_func_application(
                fn_app,
                fn_ty.return_ty.map(|x| *x),
                span,
            ));
        }

        self.init_func_app_generic_args(&fn_ty, fn_app.func.id.unwrap());

        log::debug!("resolving args of function {}", metadata.as_debug_name());
        let res = self.resolve_func_app_args(fn_app, &metadata)?;

        let app = match res {
            Ok(func) => func,
            Err(func) => {
                return Ok(Expr::new(ExprKind::Func(func)));
            }
        };

        self.finalize_func_app_generic_args(&fn_ty, app.func.id.unwrap())
            .with_span_fallback(span)?;

        // run fold again, so idents that used to point to generics get inlined
        let return_ty = fn_ty
            .return_ty
            .clone()
            .map(|ty| self.fold_type(*ty))
            .transpose()?;

        Ok(*expr_of_func_application(app, return_ty, span))
    }

    /// In PRQL, func is just an expression and does not have a name (the same way
    /// as literals don't have a name). Regardless, we want to provide name hints for functions
    /// in error messages (i.e. `std.count requires 2 arguments, found 1`), so here we infer name
    /// and annotations for functions from its declaration.
    fn gather_func_metadata(&self, func: &Expr) -> FuncMetadata {
        let mut res = FuncMetadata::default();

        let ExprKind::Ident(fq_ident) = &func.kind else {
            return res;
        };
        // let fq_ident = loop {
        //     match &func.kind {
        //         ExprKind::Ident(i) => break i,
        //         ExprKind::FuncApplication(FuncApplication { func: f, .. }) => {
        //             func = f.as_ref();
        //         }
        //         _ => return res,
        //     }
        // };

        // populate name hint
        res.name_hint = Some(fq_ident.clone());

        let decl = self.root_mod.module.get(fq_ident).unwrap();

        fn literal_as_u8(expr: Option<&Expr>) -> Option<u8> {
            Some(*expr?.kind.as_literal()?.as_integer()? as u8)
        }

        // populate implicit_closure config
        if let Some(im_clos) = decl
            .annotations
            .iter()
            .find_map(|a| a.as_func_call("implicit_closure"))
        {
            res.implicit_closure = Some(Box::new(ImplicitClosureConfig {
                param: literal_as_u8(im_clos.args.first()).unwrap(),
                this: literal_as_u8(im_clos.named_args.get("this")),
                that: literal_as_u8(im_clos.named_args.get("that")),
            }));
        }

        // populate coerce_tuple config
        if let Some(coerce_tuple) = decl
            .annotations
            .iter()
            .find_map(|a| a.as_func_call("coerce_tuple"))
        {
            res.coerce_tuple = Some(literal_as_u8(coerce_tuple.args.first()).unwrap());
        }

        res
    }

    fn init_func_app_generic_args(&mut self, fn_ty: &TyFunc, func_id: usize) {
        for generic_param in &fn_ty.generic_type_params {
            // register the generic type param in the resolver
            let generic_ident = Ident::from_path(vec![
                NS_GENERIC.to_string(),
                func_id.to_string(),
                generic_param.name.clone(),
            ]);

            let candidate = generic_param.bound.clone().map(|mut b| {
                if let TyKind::Tuple(fields) = &mut b.kind {
                    // bounds that are tuples mean "a tuple with at least these fields"
                    // so we need a global generic to track information about the other fields

                    let generic = self.init_new_global_generic("A");
                    let generic = Ty::new(TyKind::Ident(generic));
                    fields.push(TyTupleField::Unpack(Some(generic)));
                }

                (b, None)
            });

            let generic = Decl::from(DeclKind::GenericParam(candidate));
            self.root_mod.module.insert(generic_ident, generic).unwrap();
        }
    }

    fn finalize_func_app_generic_args(&mut self, fn_ty: &TyFunc, func_id: usize) -> Result<()> {
        for generic_param in &fn_ty.generic_type_params {
            let ident = Ident::from_path(vec![
                NS_GENERIC.to_string(),
                func_id.to_string(),
                generic_param.name.clone(),
            ]);

            let decl = self.root_mod.module.get_mut(&ident).unwrap();

            let DeclKind::GenericParam(inferred_type) = &mut decl.kind else {
                // this case means that we have already finalized this generic arg and should never happen
                // hack: this case does happen, because our resolution order is all over the place,
                //    so I had to add "finalize_function_generic_args" into "resolve_function_arg".
                //    This only sorta makes sense, so I want to mark this case as "will remove in the future".
                panic!()
            };

            let Some((ty, _span)) = inferred_type.take() else {
                return Err(Error::new_simple(format!(
                    "cannot determine the type {}",
                    generic_param.name
                )));
            };
            log::debug!("finalizing {ident} into {}", write_ty(&ty));
            decl.kind = DeclKind::Ty(ty);
        }
        Ok(())
    }

    /// Resolves function arguments. Will return `Err(func)` is partial application is required.
    fn resolve_func_app_args(
        &mut self,
        to_resolve: FuncApplication,
        metadata: &FuncMetadata,
    ) -> Result<Result<FuncApplication, Box<Func>>> {
        let mut app = FuncApplication {
            func: to_resolve.func,
            args: vec![Expr::new(Literal::Null); to_resolve.args.len()],
        };
        let mut partial_application_position = None;

        let func_name = &metadata.name_hint;

        let func_ty = app.func.ty.as_ref().unwrap();
        let func_ty = func_ty.kind.as_function().unwrap();
        let func_ty = func_ty.as_ref().unwrap();
        let mut param_args = itertools::zip_eq(&func_ty.params, to_resolve.args)
            .map(Box::new)
            .map(Some)
            .collect_vec();

        // pull out this and that
        let impl_cl_pos = metadata.implicit_closure.as_ref().map(|i| i.param as usize);
        let this_pos = metadata.implicit_closure.as_ref().and_then(|i| i.this);
        let that_pos = metadata.implicit_closure.as_ref().and_then(|i| i.that);

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
            let should_coerce_tuple = metadata.coerce_tuple.map_or(false, |i| i as usize == index);

            if partial_application_position.is_none() {
                if impl_cl_pos.map_or(false, |pos| pos == index) {
                    let mut scope = Scope::new();
                    if let Some(pos) = this_pos {
                        let arg = &app.args[pos as usize];
                        self.prepare_scope_of_implicit_closure_arg(&mut scope, NS_THIS, arg)?;
                    }
                    if let Some(pos) = that_pos {
                        let arg = &app.args[pos as usize];
                        self.prepare_scope_of_implicit_closure_arg(&mut scope, NS_THAT, arg)?;
                    }
                    self.scopes.push(scope);
                }

                arg = self
                    .resolve_func_app_arg(arg, param, func_name, should_coerce_tuple)?
                    .unwrap_or_else(|a| {
                        partial_application_position = Some(index);
                        a
                    });

                if impl_cl_pos.map_or(false, |pos| pos == index) {
                    self.scopes.pop();
                }
            }
            app.args[index] = arg;
        }

        Ok(if let Some(position) = partial_application_position {
            log::debug!(
                "partial application of {} at arg {position}",
                metadata.as_debug_name()
            );

            Err(extract_partial_application(app, position)?)
        } else {
            Ok(app)
        })
    }

    fn resolve_func_app_arg(
        &mut self,
        arg: Expr,
        param: &Option<Ty>,
        func_name: &Option<Ident>,
        coerce_tuple: bool,
    ) -> Result<Result<Expr, Expr>> {
        // fold
        // if param.name.starts_with("noresolve.") {
        // return Ok(Ok(arg));
        // };

        let mut arg = self.fold_expr(arg)?;

        if coerce_tuple {
            arg = self.coerce_into_tuple(arg)?;
        }

        // special case: (I forgot why this is needed)
        let expects_func = param
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
                .map(|n| format!("function {n}, one of the params")) // TODO: param name
        };
        self.validate_expr_type(&mut arg, param.as_ref(), &who)?;

        // special case: the arg is a func, finalize it generic arguments
        // (this is somewhat of a hack that is needed because of our weird resolution order)
        // if let ExprKind::FuncApplication(func) = &arg.kind {
        // self.finalize_function_generic_args(func)
        // .with_span_fallback(arg.span)?;
        // }

        Ok(Ok(arg))
    }

    /// Wraps non-tuple Exprs into a singleton Tuple.
    pub(super) fn coerce_into_tuple(&mut self, expr: Expr) -> Result<Expr> {
        let is_tuple_ty = expr.ty.as_ref().unwrap().kind.is_tuple() && !expr.kind.is_all();
        Ok(if is_tuple_ty {
            // a helpful check for a common anti-pattern
            if let Some(alias) = expr.alias {
                return Err(Error::new_simple(format!("unexpected assign to `{alias}`"))
                    .push_hint(format!("move assign into the tuple: `{{{alias} = ...}}`"))
                    .with_span(expr.span));
            }

            expr
        } else {
            let span = expr.span;
            let mut expr = Expr::new(ExprKind::Tuple(vec![expr]));
            expr.span = span;

            self.fold_expr(expr)?
        })
    }

    fn prepare_scope_of_implicit_closure_arg(
        &mut self,
        scope: &mut Scope,
        namespace: &str,
        expr: &Expr,
    ) -> Result<()> {
        let ty = expr.ty.as_ref().unwrap();

        // we expect the param to be an array of tuples, but have the type of this to be a tuple
        // here we unwrap the array and keep only the inner tuple
        let tuple_ty = match &ty.kind {
            TyKind::Array(tuple_ty) => *tuple_ty.clone(),
            TyKind::Ident(ident_of_generic) => {
                self.infer_generic_as_array(ident_of_generic, expr.span)?
            }
            _ => {
                return Err(
                    Error::new_simple("implict closure param was expected to be an array")
                        .push_hint(format!("got ty: {}", write_ty(ty))),
                );
            }
        };
        scope.values.insert(
            namespace.to_string(),
            Decl::from(DeclKind::Variable(Some(tuple_ty))),
        );
        Ok(())
    }
}

fn extract_partial_application(mut func: FuncApplication, position: usize) -> Result<Box<Func>> {
    dbg!(&func);

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
    let ExprKind::FuncApplication(arg_func) = &mut arg.kind else {
        return Err(Error::new_assert("expected func application")
            .push_hint(format!("got: {}", write_pl(arg.clone()))));
    };

    let param_name = format!("_partial_{}", arg.id.unwrap());
    let substitute_arg = Expr::new(Ident::from_path(vec![
        NS_LOCAL.to_string(),
        param_name.clone(),
    ]));
    arg_func.args.push(substitute_arg);

    // set the arg func body to the parent func
    Ok(Box::new(Func {
        return_ty: None,
        body: Box::new(Expr::new(ExprKind::FuncApplication(func))),
        params: vec![FuncParam {
            name: param_name,
            ty: None,
            default_value: None,
        }],
        named_params: Default::default(),
        generic_type_params: Default::default(),
    }))
}

fn prepare_scope_of_func(scope: &mut Scope, func: &Func) {
    for param in &func.params {
        let v = Decl {
            kind: DeclKind::Variable(param.ty.clone()),
            ..Default::default()
        };
        let param_name = param.name.split('.').last().unwrap();
        scope.values.insert(param_name.to_string(), v);
    }
}

pub fn expr_of_func_application(
    func_app: FuncApplication,
    body_ty: Option<Ty>,
    span: Option<Span>,
) -> Box<Expr> {
    let fn_ty = func_app.func.ty.as_ref().unwrap();
    let fn_ty = fn_ty.kind.as_function().unwrap();
    let fn_ty = fn_ty.as_ref().unwrap();

    let ty_func_params: Vec<_> = fn_ty.params[func_app.args.len()..].to_vec();

    let ty = if ty_func_params.is_empty() {
        body_ty
    } else {
        Some(Ty::new(TyFunc {
            params: ty_func_params,
            return_ty: body_ty.map(Box::new),
            generic_type_params: vec![],
        }))
    };

    Box::new(Expr {
        ty,
        span,
        ..Expr::new(ExprKind::FuncApplication(func_app))
    })
}
