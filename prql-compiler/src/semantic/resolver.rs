use std::collections::HashMap;
use std::iter::zip;

use anyhow::{anyhow, bail, Result};
use itertools::{Itertools, Position};

use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::{Error, Reason, Span};
use crate::semantic::scope::NS_PARAM;

use super::scope::{NS_FRAME, NS_FRAME_RIGHT};
use super::type_resolver::{resolve_type, type_of_closure, validate_type};
use super::{Context, Declaration, Frame};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(statements: Vec<Stmt>, context: Context) -> Result<(Vec<Stmt>, Context)> {
    let mut resolver = Resolver::new(context);
    let statements = resolver.fold_stmts(statements)?;

    Ok((statements, resolver.context))
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
    fn fold_stmts(&mut self, stmts: Vec<Stmt>) -> Result<Vec<Stmt>> {
        let mut res = Vec::new();

        for stmt in stmts {
            let kind = match stmt.kind {
                StmtKind::QueryDef(d) => StmtKind::QueryDef(d),
                StmtKind::FuncDef(func_def) => {
                    self.context.declare_func(func_def);
                    continue;
                }
                StmtKind::TableDef(table) => StmtKind::TableDef(self.fold_table(table)?),
                StmtKind::Pipeline(expr) => StmtKind::Pipeline(self.fold_expr(expr)?),
            };

            res.push(Stmt { kind, ..stmt })
        }
        Ok(res)
    }

    fn fold_expr(&mut self, mut node: Expr) -> Result<Expr> {
        let alias = node.alias.clone();
        let span = node.span;
        let mut r = match node.kind {
            ExprKind::Ident(ref ident) => {
                let id = self.lookup_ident(ident, node.span, &node.alias)?;
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

            ExprKind::Unary {
                op: UnOp::EqSelf,
                expr,
            } => {
                let ident = expr.kind.into_ident().map_err(|_| {
                    anyhow!("you can only use column names with self-equality operator.")
                })?;
                if ident.namespace.is_some() {
                    bail!("you cannot use namespace prefix with self-equality operator.");
                }

                let mut left = Expr::from(ExprKind::Ident(Ident {
                    namespace: Some(NS_FRAME.to_string()),
                    name: ident.name.clone(),
                }));
                left.span = node.span;
                let mut right = Expr::from(ExprKind::Ident(Ident {
                    namespace: Some(NS_FRAME_RIGHT.to_string()),
                    name: ident.name,
                }));
                right.span = node.span;
                node.kind = ExprKind::Binary {
                    left: Box::new(left),
                    op: BinOp::Eq,
                    right: Box::new(right),
                };
                node.kind = fold_expr_kind(self, node.kind)?;
                node
            }

            item => {
                node.kind = fold_expr_kind(self, item)?;
                node
            }
        };
        r.alias = alias;

        if r.span.is_none() {
            r.span = span;
        }
        if r.ty.is_none() {
            r.ty = Some(resolve_type(&r)?);
        }
        Ok(r)
    }

    fn fold_table(&mut self, TableDef { name, value, .. }: TableDef) -> Result<TableDef> {
        let id = self.context.declare_table(name.clone(), None);

        Ok(TableDef {
            id: Some(id),
            name,
            value: Box::new(self.fold_expr(*value)?),
        })
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
    pub fn lookup_ident(
        &mut self,
        ident: &Ident,
        span: Option<Span>,
        alias: &Option<String>,
    ) -> Result<usize> {
        Ok(match self.namespace {
            Namespace::Tables => self.context.declare_table(ident.to_string(), alias.clone()),
            Namespace::FunctionsColumns => {
                let res = self.context.lookup_ident(ident, span);

                match res {
                    Ok(id) => id,
                    Err(e) => {
                        dbg!(&self.context);
                        bail!(Error::new(Reason::Simple(e)).with_span(span))
                    }
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
                Ok(transform) => {
                    // this function call is a transform, append it to the pipeline

                    let ty = Ty::Table(transform.infer_type()?);
                    let mut expr = Expr::from(ExprKind::TransformCall(transform));
                    expr.ty = Some(ty);
                    expr.span = span;
                    expr
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
        named_args: HashMap<String, Expr>,
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
            args: vec![Expr::null(); to_resolve.args.len()],
            named_args: Vec::with_capacity(to_resolve.named_args.len()),
            ..to_resolve
        };

        let func_name = closure.name.as_deref();

        {
            // positional args

            let (tables, other): (Vec<_>, Vec<_>) = zip(&closure.params, to_resolve.args)
                .enumerate()
                .partition(|(_, (param, _))| {
                    let is_table = param
                        .ty
                        .as_ref()
                        .map(|t| matches!(t, Ty::Table(_)))
                        .unwrap_or_default();

                    is_table
                });
            closure.args = vec![Expr::null(); closure.params.len()];

            self.context.scope.push_namespace(NS_FRAME);
            self.context.scope.push_namespace(NS_FRAME_RIGHT);

            // resolve tables
            for pos in tables.into_iter().with_position() {
                let is_last = matches!(pos, Position::Last(_) | Position::Only(_));
                let (index, (param, arg)) = pos.into_inner();

                // just fold the argument alone
                let arg = self.fold_and_type_check(arg, param, func_name)?;

                // add table's frame into scope
                if let Some(Ty::Table(frame)) = &arg.ty {
                    if let Some(alias) = arg.alias.clone() {
                        self.context.scope.add_frame_columns(frame, &alias);
                    }

                    if is_last {
                        self.context.scope.add_frame_columns(frame, NS_FRAME);
                    } else {
                        self.context.scope.add_frame_columns(frame, NS_FRAME_RIGHT);
                    }
                }

                closure.args[index] = arg;
            }

            // resolve other positional
            for (index, (param, arg)) in other {
                let arg = match arg.kind {
                    // if this is a list, fold one by one
                    ExprKind::List(items) => {
                        let mut res = Vec::with_capacity(items.len());
                        for item in items {
                            let mut item = self.fold_and_type_check(item, param, func_name)?;

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
                    _ => self.fold_and_type_check(arg, param, func_name)?,
                };

                closure.args[index] = arg;
            }

            self.context.scope.pop_namespace(NS_FRAME);
            self.context.scope.pop_namespace(NS_FRAME_RIGHT);
        }

        {
            // named args
            for (index, arg) in to_resolve.named_args.into_iter().enumerate() {
                if let Some(arg) = arg {
                    let param = &closure.named_params[index];

                    let arg = self.fold_and_type_check(arg, param, func_name)?;
                    closure.named_args.push(Some(arg));
                } else {
                    closure.named_args.push(None);
                }
            }
        }

        Ok(closure)
    }

    fn fold_and_type_check(
        &mut self,
        arg: Expr,
        param: &FuncParam,
        func_name: Option<&str>,
    ) -> Result<Expr> {
        let mut arg = self.fold_within_namespace(arg, &param.name)?;

        // validate type
        let param_ty = param.ty.as_ref().unwrap_or(&Ty::Infer);
        let assumed_ty = validate_type(&arg, param_ty, || {
            func_name.map(|n| format!("function `{n}`, param `{}`", param.name))
        })?;
        arg.ty = Some(assumed_ty);

        Ok(arg)
    }

    fn fold_within_namespace(&mut self, expr: Expr, param_name: &str) -> Result<Expr> {
        let prev_namespace = self.namespace;

        if param_name.starts_with("noresolve.") {
            return Ok(expr);
        } else if param_name.starts_with("tables.") {
            self.namespace = Namespace::Tables;
        } else {
            self.namespace = Namespace::FunctionsColumns;
        };

        let res = self.fold_expr(expr);
        self.namespace = prev_namespace;
        res
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
    use anyhow::Result;
    use insta::{assert_display_snapshot, assert_yaml_snapshot};

    use crate::ast::{Expr, Ty};
    use crate::semantic::resolve_only;
    use crate::utils::IntoOnly;
    use crate::{compile, parse};

    fn parse_and_resolve(query: &str) -> Result<Expr> {
        let (stmts, _) = resolve_only(parse(query)?, None)?;

        Ok(stmts.into_only()?.kind.into_pipeline()?)
    }

    fn resolve_type(query: &str) -> Result<Ty> {
        Ok(parse_and_resolve(query)?.ty.unwrap_or_default())
    }

    fn resolve_derive(query: &str) -> Result<Vec<Expr>> {
        let expr = parse_and_resolve(query)?;
        let derive = expr.kind.into_transform_call()?;
        let (assigns, _) = derive.kind.into_derive()?;
        Ok(assigns)
    }

    #[test]
    #[ignore]
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

    #[test]
    fn test_variables_1() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            from employees
            derive [
                gross_salary = salary + payroll_tax,
                gross_cost =   gross_salary + benefits_cost
            ]
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_non_existent_function() {
        parse_and_resolve(r#"from mytable | filter (myfunc col1)"#).unwrap_err();
    }

    #[test]
    fn test_functions_1() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            func subtract a b -> a - b

            from employees
            derive [
                net_salary = subtract gross_salary tax
            ]
            "#
        )
        .unwrap());
    }

    #[test]
    #[ignore]
    fn test_functions_nested() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            func lag_day x -> s"lag_day_todo({x})"
            func ret x dividend_return ->  x / (lag_day x) - 1 + dividend_return

            from a
            select (ret b c)
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_functions_pipeline() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            from a
            derive one = (foo | sum)
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_derive(
            r#"
            func plus_one x -> x + 1
            func plus x y -> x + y

            from a
            derive [b = (sum foo | plus_one | plus 2)]
            "#
        )
        .unwrap());
    }
    #[test]
    fn test_named_args() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            func add x to:1 -> x + to

            from foo_table
            derive [
                added = add bar to:3,
                added_default = add bar
            ]
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_frames_and_names() {
        assert_yaml_snapshot!(resolve_type(
            r#"
            from orders
            select [customer_no, gross, tax, gross - tax]
            take 20
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_type(
            r#"
            from table_1
            join customers [~customer_no]
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_type(
            r#"
            from employees
            join salaries [~emp_no]
            group [emp_no, gender] (
                aggregate [
                    emp_salary = average salary
                ]
            )
            "#
        )
        .unwrap());
    }
}
