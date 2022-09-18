use std::collections::HashMap;
use std::iter::zip;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::{Error, Reason, Span};
use crate::semantic::scope::NS_PARAM;

use super::scope::NS_FRAME;
use super::type_resolver::{resolve_type, type_of_closure, validate_type};
use super::{Context, Declaration, Frame};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(statements: Vec<Stmt>, context: Context) -> Result<(Query, Context)> {
    let mut resolver = Resolver::new(context);

    let query = resolver.fold_statements(statements)?;

    Ok((query, resolver.context))
}

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,

    namespace: Namespace,

    /// Sometimes ident closures must be resolved and sometimes not. See [test::test_func_call_resolve].
    in_func_call_name: bool,
}

impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            context,
            namespace: Namespace::FunctionsColumns,
            in_func_call_name: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Namespace {
    FunctionsColumns,
    Tables,
}

impl AstFold for Resolver {
    fn fold_expr(&mut self, mut node: Expr) -> Result<Expr> {
        let alias = node.alias.clone();
        let mut r = match node.kind {
            ExprKind::Ident(ref ident) => {
                let id = self.lookup_name(ident, node.span, &node.alias)?;
                node.declared_at = Some(id);

                let decl = self.context.declarations.get(id);

                match decl {
                    // convert ident to function without args
                    Declaration::Function(func_def) => {
                        let closure = closure_of_func_def(func_def);

                        if self.in_func_call_name {
                            Expr::from(ExprKind::Closure(closure))
                        } else {
                            self.fold_function(closure, vec![], HashMap::new(), node.span)?
                        }
                    }

                    // init type for tables
                    Declaration::Table(_) => Expr {
                        ty: Some(Ty::Table(Frame::unknown(id))),
                        ..node
                    },

                    Declaration::Expression(expr) => *expr.clone(),

                    _ => node,
                }
            }

            ExprKind::FuncCall(FuncCall {
                name,
                args,
                named_args,
            }) => {
                // fold name (or closure)
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                let name = self.fold_expr(*name)?;
                self.in_func_call_name = old;

                let closure = name.try_cast(|n| n.into_closure(), None, "a function")?;

                // fold function
                self.fold_function(closure, args, named_args, node.span)?
            }

            ExprKind::Pipeline(Pipeline { mut exprs }) => {
                let mut value = exprs.remove(0);
                for expr in exprs {
                    let span = expr.span;

                    value = Expr::from(ExprKind::FuncCall(FuncCall {
                        name: Box::new(expr),
                        args: vec![value],
                        named_args: HashMap::new(),
                    }));
                    value.span = span;
                }
                self.fold_expr(value)?
            }

            ExprKind::Closure(closure) => {
                self.fold_function(closure, vec![], HashMap::new(), node.span)?
            }

            item => {
                node.kind = fold_expr_kind(self, item)?;
                node
            }
        };
        r.alias = alias;

        if r.ty.is_none() {
            r.ty = Some(resolve_type(&r)?)
        }
        Ok(r)
    }
}

fn closure_of_func_def(func_def: &FuncDef) -> Closure {
    Closure {
        name: Some(func_def.name.clone()),
        body: func_def.body.clone(),

        args: vec![],
        params: func_def.positional_params.clone(),

        named_args: vec![],
        named_params: func_def.named_params.clone(),

        env: HashMap::default(),
    }
}

impl Resolver {
    pub fn lookup_name(
        &mut self,
        name: &str,
        span: Option<Span>,
        alias: &Option<String>,
    ) -> Result<usize> {
        Ok(match self.namespace {
            Namespace::Tables => self.context.declare_table(name.to_string(), alias.clone()),
            Namespace::FunctionsColumns => {
                let res = self.context.lookup_name(name, span);

                match res {
                    Ok(id) => id,
                    Err(e) => bail!(Error::new(Reason::Simple(e)).with_span(span)),
                }
            }
        })
    }

    fn fold_function(
        &mut self,
        closure: Closure,
        args: Vec<Expr>,
        named_args: HashMap<String, Expr>,
        span: Option<Span>,
    ) -> Result<Expr, anyhow::Error> {
        let closure = self.apply_args_to_closure(closure, args, named_args)?;
        let args_len = closure.args.len();

        let enough_args = closure.args.len() >= closure.params.len();

        let mut r = if enough_args {
            
            let closure = self.resolve_function_args(closure)?;

            // evaluate
            match super::transforms::cast_transform(self, closure)? {
                Ok((preceding_pipeline, transform)) => {
                    // this function call is a transform, append it to the pipeline

                    let mut pipeline = preceding_pipeline
                        .unwrap_or_else(|| ExprKind::ResolvedPipeline(vec![]).into());

                    let ty = pipeline.ty.as_ref().and_then(|t| t.as_table());
                    let frame_before = ty.cloned().unwrap_or_default();
                    let frame_after = transform.apply_to(frame_before)?;
                    pipeline.ty = Some(Ty::Table(frame_after.clone()));

                    let transform = Transform {
                        kind: transform,
                        ty: frame_after,
                        is_complex: false,
                        span,
                    };

                    if let Some(transforms) = pipeline.kind.as_resolved_pipeline_mut() {
                        transforms.push(transform);
                    }

                    pipeline
                }
                Err(closure) => {
                    // this function call is not a transform, proceed with materialization

                    let (func_env, body) = env_of_closure(closure);

                    self.context.scope.push_namespace(NS_PARAM);
                    self.context.insert_decls(NS_PARAM, func_env);

                    // fold again, to resolve inner variables & functions
                    let body = self.fold_expr(body)?;

                    // remove param decls
                    let func_env = self.context.take_decls(NS_PARAM);

                    if let ExprKind::Closure(mut inner_closure) = body.kind {
                        // body couldn't been resolved - construct a closure to be evaluated later

                        inner_closure.env = func_env;

                        let (got, missing) =
                            inner_closure.params.split_at(inner_closure.args.len());
                        let missing = missing.to_vec();
                        inner_closure.params = got.to_vec();

                        Expr::from(ExprKind::Closure(Closure {
                            name: None,
                            args: vec![],
                            params: missing,
                            named_args: vec![],
                            named_params: vec![],
                            body: Box::new(Expr::from(ExprKind::Closure(inner_closure))),
                            env: HashMap::new(),
                        }))
                    } else {
                        // resolved, return result
                        body
                    }
                }
            }
        } else {
            // not enough arguments: construct a func closure

            let mut ty = type_of_closure(&closure);
            ty.args = ty.args[args_len..].to_vec();
            ty.named.clear();

            let mut node = Expr::from(ExprKind::Closure(closure));
            node.ty = Some(Ty::Function(ty));

            node
        };
        r.span = span;
        Ok(r)
    }

    fn apply_args_to_closure(
        &mut self,
        mut closure: Closure,
        args: Vec<Expr>,
        named_args: HashMap<Ident, Expr>,
    ) -> Result<Closure> {
        for arg in args {
            closure.args.push(arg);
        }

        // named arguments are consumed only by the first function (non-curry)
        if !closure.named_args.is_empty() {
            if !named_args.is_empty() {
                bail!("function curry cannot accept named arguments");
            }
        } else {
            closure.named_args = closure.named_params.iter().map(|_| None).collect();
            for (name, arg) in named_args {
                let (index, _) = closure
                    .named_params
                    .iter()
                    .find_position(|p| p.name == name)
                    .ok_or_else(|| anyhow::anyhow!("unknown named argument"))?;

                closure.named_args[index] = Some(arg);
            }
        }

        Ok(closure)
    }

    fn resolve_function_args(&mut self, to_resolve: Closure) -> Result<Closure> {
        let mut closure = Closure {
            args: Vec::with_capacity(to_resolve.args.len()),
            named_args: Vec::with_capacity(to_resolve.named_args.len()),
            ..to_resolve
        };

        let func_name = closure.name.as_deref();

        {
            // positional args
            // use reverse order because frame must be resolved first so it can be added to scope
            let mut frame_in_scope = false;
            for (index, arg) in to_resolve.args.into_iter().enumerate().rev() {
                let param = &closure.params[index];

                let arg = match arg.kind {
                    // if this is a list, fold one by one
                    ExprKind::List(items) => {
                        let mut res = Vec::with_capacity(items.len());
                        for item in items {
                            let mut item = self.resolve_function_arg(item, param, func_name)?;

                            // add aliased columns into scope
                            if let Some(alias) = item.alias.clone() {
                                self.context.declare_as_ident(&mut item);

                                let id = item.declared_at.unwrap();
                                self.context.scope.add(NS_FRAME, alias, id);
                            }
                            res.push(item);
                        }
                        Expr {
                            kind: ExprKind::List(res),
                            ..arg
                        }
                    }

                    // just fold the argument alone
                    _ => self.resolve_function_arg(arg, param, func_name)?,
                };

                // add table's frame into scope
                if !frame_in_scope {
                    if let Some(Ty::Table(frame)) = &arg.ty {
                        self.context.scope.add_frame_columns(frame);
                        frame_in_scope = true;
                    }
                }

                // push front (because of reverse resolve order)
                closure.args.insert(0, arg);
            }

            if frame_in_scope {
                self.context.scope.drop(NS_FRAME);
            }
        }

        {
            // named args
            for (index, arg) in to_resolve.named_args.into_iter().enumerate() {
                if let Some(arg) = arg {
                    let param = &closure.named_params[index];

                    let arg = self.resolve_function_arg(arg, param, func_name)?;
                    closure.named_args.push(Some(arg));
                } else {
                    closure.named_args.push(None);
                }
            }
        }

        Ok(closure)
    }

    fn resolve_function_arg(
        &mut self,
        arg: Expr,
        param: &FuncParam,
        func_name: Option<&str>,
    ) -> Result<Expr> {
        let mut arg = match arg.kind {
            // if this is a list, fold one by one
            ExprKind::List(items) => {
                let mut res = Vec::with_capacity(items.len());
                for item in items {
                    let mut item = self.fold_within_namespace(item, &param.ty)?;

                    // add aliased columns into scope
                    if let Some(alias) = item.alias.clone() {
                        self.context.declare_as_ident(&mut item);

                        let id = item.declared_at.unwrap();
                        self.context.scope.add(NS_FRAME, alias, id);
                    }
                    res.push(item);
                }
                Expr {
                    kind: ExprKind::List(res),
                    ..arg
                }
            }

            // just fold the argument alone
            _ => self.fold_within_namespace(arg, &param.ty)?,
        };

        // validate type
        let param_ty = param.ty.as_ref().unwrap_or(&Ty::Infer);
        let assumed_ty = validate_type(&arg, param_ty, || func_name.map(|n| n.to_string()))?;
        arg.ty = Some(assumed_ty);

        Ok(arg)
    }

    fn fold_within_namespace(&mut self, expr: Expr, ty: &Option<Ty>) -> Result<Expr> {
        let prev_namespace = self.namespace;
        let res = match ty.as_ref() {
            Some(Ty::BuiltinKeyword) => Ok(expr),
            Some(Ty::Table(_)) => {
                self.namespace = Namespace::Tables;
                self.fold_expr(expr)
            }
            _ => {
                self.namespace = Namespace::FunctionsColumns;
                self.fold_expr(expr)
            }
        };
        self.namespace = prev_namespace;
        res
    }

    /// fold statements and extract results into a [Query]
    fn fold_statements(&mut self, stmts: Vec<Stmt>) -> Result<Query> {
        let mut def = None;
        let mut main_pipeline = Vec::new();
        let mut tables = Vec::new();

        for stmt in stmts {
            match stmt.kind {
                StmtKind::QueryDef(d) => def = Some(d),
                StmtKind::FuncDef(func_def) => {
                    self.context.declare_func(func_def);
                }
                StmtKind::TableDef(table) => {
                    let table = self.fold_table(table)?;
                    let table = Table {
                        id: table.id,
                        name: table.name,
                        pipeline: table.pipeline.kind.into_resolved_pipeline()?,
                    };
                    tables.push(table);
                }
                StmtKind::Pipeline(exprs) => {
                    let expr = self.fold_expr(ExprKind::Pipeline(Pipeline { exprs }).into())?;

                    match expr.kind {
                        ExprKind::ResolvedPipeline(transforms) => {
                            main_pipeline = transforms;
                        }
                        _ => {
                            bail!(Error::new(Reason::Expected {
                                who: None,
                                expected: "pipeline that resolves to a table".to_string(),
                                found: format!("`{expr}`")
                            })
                            .with_help("are you missing `from` statement?")
                            .with_span(stmt.span))
                        }
                    }
                }
            }
        }
        Ok(Query {
            def: def.unwrap_or_default(),
            main_pipeline,
            tables,
        })
    }
}

fn env_of_closure(closure: Closure) -> (HashMap<String, Declaration>, Expr) {
    let mut func_env = HashMap::new();

    for (param, arg) in zip(closure.named_params, closure.named_args) {
        let value = arg.unwrap_or_else(|| param.default_value.unwrap());
        func_env.insert(param.name.clone(), Declaration::Expression(Box::new(value)));
    }
    for (param, arg) in zip(&closure.params, closure.args) {
        func_env.insert(param.name.clone(), Declaration::Expression(Box::new(arg)));
    }
    (func_env, *closure.body)
}

#[cfg(test)]
mod test {
    use insta::assert_display_snapshot;

    use crate::compile;

    #[test]
    fn test_func_call_resolve() {
        assert_display_snapshot!(compile(r#"
        from employees
        aggregate [
          count non_null:salary,
          count,
        ]
        "#).unwrap(),
            @r###"
        SELECT
          COUNT(salary),
          COUNT(*)
        FROM
          employees
        "###
        );
    }
}
