use std::collections::HashMap;
use std::iter::zip;

use anyhow::{anyhow, bail, Result};
use itertools::{Itertools, Position};

use crate::ast::pl::{fold::*, *};
use crate::error::{Error, Reason, Span};
use crate::utils::IdGenerator;

use super::context::{Context, Decl, DeclKind, TableColumn};
use super::module::{Module, NS_FRAME, NS_FRAME_RIGHT, NS_PARAM};
use super::reporting::debug_call_tree;
use super::transforms::Flattener;
use super::type_resolver::{resolve_type, type_of_closure, validate_type};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(stmts: Vec<Stmt>, context: Context) -> Result<(Vec<Stmt>, Context)> {
    let mut resolver = Resolver::new(context);
    let stmts = resolver.fold_stmts(stmts)?;

    Ok((stmts, resolver.decls))
}

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver {
    pub decls: Context,

    default_namespace: Option<String>,

    /// Sometimes ident closures must be resolved and sometimes not. See [test::test_func_call_resolve].
    in_func_call_name: bool,

    pub(super) id: IdGenerator<usize>,
}

impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            decls: context,
            default_namespace: None,
            in_func_call_name: false,
            id: IdGenerator::new(),
        }
    }
}

impl AstFold for Resolver {
    fn fold_stmts(&mut self, stmts: Vec<Stmt>) -> Result<Vec<Stmt>> {
        let mut res = Vec::new();

        for mut stmt in stmts {
            stmt.id = Some(self.id.gen());
            if let Some(span) = stmt.span {
                self.decls.span_map.insert(stmt.id.unwrap(), span);
            }

            let kind = match stmt.kind {
                StmtKind::QueryDef(d) => StmtKind::QueryDef(d),
                StmtKind::FuncDef(func_def) => {
                    self.decls.declare_func(func_def, stmt.id);
                    continue;
                }
                StmtKind::TableDef(table_def) => {
                    let table_def = self.fold_table(table_def)?;
                    let table_def = TableDef {
                        value: Box::new(Flattener::fold(*table_def.value)),
                        ..table_def
                    };
                    self.decls.declare_table(table_def, stmt.id);
                    continue;
                }
                StmtKind::Pipeline(expr) => {
                    let x = Flattener::fold(self.fold_expr(*expr)?);
                    StmtKind::Pipeline(Box::new(x))
                }
            };

            res.push(Stmt { kind, ..stmt })
        }
        Ok(res)
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if node.id.is_some() && !matches!(node.kind, ExprKind::Closure(_)) {
            return Ok(node);
        }

        let id = self.id.gen();
        let alias = node.alias.clone();
        let span = node.span;

        if let Some(span) = span {
            self.decls.span_map.insert(id, span);
        }

        log::trace!("folding expr {node:?}");

        let mut r = match node.kind {
            ExprKind::Ident(ident) => {
                log::debug!("resolving ident {ident}...");
                let fq_ident = self.resolve_ident(&ident, node.span)?;
                log::debug!("... resolved to {fq_ident}");
                let entry = self.decls.root_mod.get(&fq_ident).unwrap();
                log::debug!("... which is {entry}");

                match &entry.kind {
                    // convert ident to function without args
                    DeclKind::FuncDef(func_def) => {
                        let closure = closure_of_func_def(func_def, fq_ident);

                        if self.in_func_call_name {
                            Expr::from(ExprKind::Closure(Box::new(closure)))
                        } else {
                            self.fold_function(closure, vec![], HashMap::new(), node.span)?
                        }
                    }

                    DeclKind::Column(target_id) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        target_id: Some(*target_id),
                        ..node
                    },

                    DeclKind::TableDef { frame, .. } => {
                        let alias = node.alias.unwrap_or_else(|| ident.name.clone());

                        let instance_frame = Frame {
                            inputs: vec![FrameInput {
                                id,
                                name: alias.clone(),
                                table: fq_ident.clone(),
                            }],
                            columns: frame
                                .columns
                                .iter()
                                .map(|col| match col {
                                    TableColumn::Wildcard => FrameColumn::Wildcard {
                                        input_name: alias.clone(),
                                    },
                                    TableColumn::Single(name) => FrameColumn::Single {
                                        name: name.clone().map(|name| Ident {
                                            name,
                                            path: vec![alias.clone()],
                                        }),
                                        expr_id: id,
                                    },
                                })
                                .collect(),
                        };

                        log::debug!("instanced table {fq_ident} as {instance_frame:?}");

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(Ty::Table(instance_frame)),
                            alias: None,
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => expr.as_ref().clone(),

                    DeclKind::NoResolve => Expr {
                        kind: ExprKind::Ident(ident),
                        ..node
                    },

                    _ => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ..node
                    },
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
                self.fold_function(*closure, args, named_args, node.span)?
            }

            ExprKind::Pipeline(pipeline) => self.resolve_pipeline(pipeline)?,

            ExprKind::Closure(closure) => {
                self.fold_function(*closure, vec![], HashMap::new(), node.span)?
            }

            ExprKind::Unary {
                op: UnOp::EqSelf,
                expr,
            } => {
                let kind = self.resolve_eq_self(*expr, span)?;
                Expr { kind, ..node }
            }

            item => Expr {
                kind: fold_expr_kind(self, item)?,
                ..node
            },
        };
        r.id = Some(id);
        r.alias = alias;

        if r.span.is_none() {
            r.span = span;
        }
        if r.ty.is_none() {
            r.ty = Some(resolve_type(&r)?);
        }
        Ok(r)
    }
}

fn closure_of_func_def(func_def: &FuncDef, fq_ident: Ident) -> Closure {
    Closure {
        name: Some(fq_ident),
        body: func_def.body.clone(),
        body_ty: func_def.return_ty.clone(),

        params: func_def.positional_params.clone(),
        named_params: func_def.named_params.clone(),

        args: vec![],

        env: HashMap::default(),
    }
}

impl Resolver {
    fn resolve_pipeline(&mut self, Pipeline { mut exprs }: Pipeline) -> Result<Expr> {
        let mut value = exprs.remove(0);
        value = self.fold_expr(value)?;

        let closure_param = if let ExprKind::Closure(closure) = &mut value.kind {
            let param = "_pip_val";
            let value = Expr::from(ExprKind::Ident(Ident::from_name(param)));
            closure.args.push(value);
            Some(param)
        } else {
            None
        };

        for expr in exprs {
            let span = expr.span;

            value = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(expr),
                args: vec![value],
                named_args: HashMap::new(),
            }));
            value.span = span;
        }

        if let Some(closure_param) = closure_param {
            value = Expr::from(ExprKind::Closure(Box::new(Closure {
                name: None,
                body: Box::new(value),
                body_ty: None,

                args: vec![],
                params: vec![FuncParam {
                    name: closure_param.to_string(),
                    default_value: None,
                    ty: None,
                }],
                named_params: vec![],
                env: HashMap::new(),
            })));
        }

        if log::log_enabled!(log::Level::Debug) {
            let (v, tree) = debug_call_tree(value);
            value = v;
            log::debug!("unpacked pipeline to following call tree: \n{tree}");
        }

        self.fold_expr(value)
    }

    pub fn resolve_ident(&mut self, ident: &Ident, span: Option<Span>) -> Result<Ident> {
        let res = if ident.path.is_empty() && self.default_namespace.is_some() {
            let defaulted = Ident {
                path: vec![self.default_namespace.clone().unwrap()],
                name: ident.name.clone(),
            };
            self.decls.resolve_ident(&defaulted)
        } else {
            self.decls.resolve_ident(ident)
        };

        res.map_err(|e| {
            log::debug!("cannot resolve, context={:#?}", self.decls);
            anyhow!(Error::new(Reason::Simple(e)).with_span(span))
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

        log::debug!("{} >= {}", closure.args.len(), closure.params.len());
        let enough_args = closure.args.len() >= closure.params.len();

        let mut r = if enough_args {
            if log::log_enabled!(log::Level::Debug) {
                let name = closure
                    .name
                    .clone()
                    .unwrap_or_else(|| Ident::from_name("<unnamed>"));
                log::debug!("resolving args of function {}", name);
            }
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
                    let needs_window = Some(Ty::column()) <= closure.body_ty;

                    let (func_env, body) = env_of_closure(closure);

                    self.decls.root_mod.stack_push(NS_PARAM, func_env);
                    log::debug!("stack_push");

                    // fold again, to resolve inner variables & functions
                    let body = self.fold_expr(body)?;

                    // remove param decls
                    log::debug!("stack_pop");
                    let func_env = self.decls.root_mod.stack_pop(NS_PARAM).unwrap();

                    let mut res = if let ExprKind::Closure(mut inner_closure) = body.kind {
                        // body couldn't been resolved - construct a closure to be evaluated later

                        inner_closure.env = func_env.into_exprs();

                        let (got, missing) =
                            inner_closure.params.split_at(inner_closure.args.len());
                        let missing = missing.to_vec();
                        inner_closure.params = got.to_vec();

                        Expr::from(ExprKind::Closure(Box::new(Closure {
                            name: None,
                            args: vec![],
                            params: missing,
                            named_params: vec![],
                            body: Box::new(Expr::from(ExprKind::Closure(inner_closure))),
                            body_ty: None,
                            env: HashMap::new(),
                        })))
                    } else {
                        // resolved, return result
                        body
                    };

                    res.needs_window = needs_window;
                    res
                }
            }
        } else {
            // not enough arguments: construct a func closure

            let mut ty = type_of_closure(&closure);
            ty.args = ty.args[args_len..].to_vec();

            let mut node = Expr::from(ExprKind::Closure(Box::new(closure)));
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
        mut named_args: HashMap<String, Expr>,
    ) -> Result<Closure> {
        // named arguments are consumed only by the first function

        // named
        for mut param in closure.named_params.drain(..) {
            let param_name = param.name.split('.').last().unwrap_or(&param.name);
            let default = param.default_value.take().unwrap();

            let arg = named_args.remove(param_name).unwrap_or(default);

            closure.args.push(arg);
            closure.params.insert(closure.args.len() - 1, param);
        }
        if let Some((name, _)) = named_args.into_iter().next() {
            // TODO: report all remaining named_args as separate errors
            anyhow::bail!(
                "unknown named argument `{name}` to closure {:?}",
                closure.name
            )
        }

        // positional
        closure.args.extend(args);
        Ok(closure)
    }

    fn resolve_function_args(&mut self, to_resolve: Closure) -> Result<Closure> {
        let mut closure = Closure {
            args: vec![Expr::null(); to_resolve.args.len()],
            ..to_resolve
        };

        let func_name = &closure.name;

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

        let has_tables = !tables.is_empty();

        // resolve tables
        if has_tables {
            self.decls.root_mod.shadow(NS_FRAME);
            self.decls.root_mod.shadow(NS_FRAME_RIGHT);

            for pos in tables.into_iter().with_position() {
                let is_last = matches!(pos, Position::Last(_) | Position::Only(_));
                let (index, (param, arg)) = pos.into_inner();

                // just fold the argument alone
                let arg = self.fold_and_type_check(arg, param, func_name)?;
                log::debug!("resolved arg to {}", arg.kind.as_ref());

                // add table's frame into scope
                if let Some(Ty::Table(frame)) = &arg.ty {
                    if is_last {
                        self.decls.root_mod.insert_frame(frame, NS_FRAME);
                    } else {
                        self.decls.root_mod.insert_frame(frame, NS_FRAME_RIGHT);
                    }
                }

                closure.args[index] = arg;
            }
        }

        // resolve other positional
        for (index, (param, arg)) in other {
            let arg = match arg.kind {
                // if this is a list, fold one by one
                ExprKind::List(items) => {
                    let mut res = Vec::with_capacity(items.len());
                    for item in items {
                        let item = self.fold_and_type_check(item, param, func_name)?;

                        // add aliased columns into scope
                        if let Some(alias) = item.alias.clone() {
                            let id = item.id.unwrap();
                            self.decls.root_mod.insert_frame_col(NS_FRAME, alias, id);
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

        if has_tables {
            self.decls.root_mod.unshadow(NS_FRAME);
            self.decls.root_mod.unshadow(NS_FRAME_RIGHT);
        }

        Ok(closure)
    }

    fn fold_and_type_check(
        &mut self,
        arg: Expr,
        param: &FuncParam,
        func_name: &Option<Ident>,
    ) -> Result<Expr> {
        let mut arg = self.fold_within_namespace(arg, &param.name)?;

        // don't validate types of unresolved exprs
        if arg.ty.is_some() {
            // validate type
            let param_ty = param.ty.as_ref().unwrap_or(&Ty::Infer);
            let assumed_ty = validate_type(&arg, param_ty, || {
                func_name
                    .as_ref()
                    .map(|n| format!("function {n}, param `{}`", param.name))
            })?;
            arg.ty = Some(assumed_ty);
        }

        Ok(arg)
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

    fn resolve_eq_self(&mut self, expr: Expr, span: Option<Span>) -> Result<ExprKind> {
        let ident = expr
            .kind
            .into_ident()
            .map_err(|_| anyhow!("you can only use column names with self-equality operator."))?;
        if !ident.path.is_empty() {
            bail!("you cannot use namespace prefix with self-equality operator.");
        }
        let mut left = Expr::from(ExprKind::Ident(Ident {
            path: vec![NS_FRAME.to_string()],
            name: ident.name.clone(),
        }));
        left.span = span;
        let mut right = Expr::from(ExprKind::Ident(Ident {
            path: vec![NS_FRAME_RIGHT.to_string()],
            name: ident.name,
        }));
        right.span = span;
        let kind = ExprKind::Binary {
            left: Box::new(left),
            op: BinOp::Eq,
            right: Box::new(right),
        };
        let kind = fold_expr_kind(self, kind)?;
        Ok(kind)
    }
}

fn env_of_closure(closure: Closure) -> (Module, Expr) {
    let mut func_env = Module::default();

    for (param, arg) in zip(closure.params, closure.args) {
        let v = Decl {
            declared_at: arg.id,
            kind: DeclKind::Expr(Box::new(arg)),
        };
        let param_name = param.name.split('.').last().unwrap();
        func_env.names.insert(param_name.to_string(), v);
    }

    (func_env, *closure.body)
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::{assert_display_snapshot, assert_yaml_snapshot};

    use crate::ast::pl::{Expr, Ty};
    use crate::semantic::resolve_only;
    use crate::utils::IntoOnly;
    use crate::{compile, parse};

    fn parse_and_resolve(query: &str) -> Result<Expr> {
        let (stmts, _) = resolve_only(parse(query)?, None)?;

        Ok(*stmts.into_only()?.kind.into_pipeline()?)
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
            join customers [==customer_no]
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_type(
            r#"
            from employees
            join salaries [==emp_no]
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
