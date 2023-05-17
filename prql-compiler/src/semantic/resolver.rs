use std::collections::HashMap;
use std::iter::zip;

use anyhow::{anyhow, bail, Result};
use itertools::{Itertools, Position};

use crate::ast::pl::{fold::*, *};
use crate::error::{Error, Reason, Span, WithErrorInfo};
use crate::semantic::transforms::coerce_and_flatten;
use crate::semantic::{static_analysis, NS_PARAM};
use crate::utils::IdGenerator;

use super::context::{Context, Decl, DeclKind};
use super::module::Module;
use super::reporting::debug_call_tree;
use super::transforms::{self, Flattener};
use super::type_resolver::{self, infer_type, type_of_closure};
use super::{NS_FRAME, NS_FRAME_RIGHT, NS_STD};

/// Runs semantic analysis on the query, using current state.
///
/// Note that this removes function declarations from AST and saves them as current context.
pub fn resolve(stmts: Vec<Stmt>, path: Vec<String>, context: Context) -> Result<Context> {
    let mut resolver = Resolver::new(context);
    resolver.current_module_path = path;

    forbid_invalid_stmts(&stmts)?;

    resolver.fold_statements(stmts)?;

    Ok(resolver.context)
}

fn forbid_invalid_stmts(stmts: &Vec<Stmt>) -> Result<()> {
    // Because I added parsing for module declarations for development purposes,
    // but we don't intend to support them, we forbid them here.

    for stmt in stmts {
        if let StmtKind::ModuleDef(_) = stmt.kind {
            return Err(
                Error::new_simple("explicit module declarations are not allowed")
                    .with_span(stmt.span)
                    .into(),
            );
        }
    }
    Ok(())
}

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver {
    pub context: Context,

    current_module_path: Vec<String>,

    default_namespace: Option<String>,

    /// Sometimes ident closures must be resolved and sometimes not. See [test::test_func_call_resolve].
    in_func_call_name: bool,

    pub(super) id: IdGenerator<usize>,
}

impl Resolver {
    fn new(context: Context) -> Self {
        Resolver {
            context,
            current_module_path: Vec::new(),
            default_namespace: None,
            in_func_call_name: false,
            id: IdGenerator::new(),
        }
    }

    fn fold_statements(&mut self, stmts: Vec<Stmt>) -> Result<()> {
        for mut stmt in stmts {
            stmt.id = Some(self.id.gen());
            if let Some(span) = stmt.span {
                self.context.span_map.insert(stmt.id.unwrap(), span);
            }

            let ident = Ident {
                path: self.current_module_path.clone(),
                name: stmt.name.clone(),
            };

            let mut def = match stmt.kind {
                StmtKind::QueryDef(d) => {
                    let decl = DeclKind::QueryDef(d);
                    self.context
                        .declare(ident, decl, stmt.id)
                        .with_span(stmt.span)?;
                    continue;
                }
                StmtKind::VarDef(var_def) => self.fold_var_def(var_def)?,
                StmtKind::TypeDef(ty_def) => {
                    let mut var_def = VarDef {
                        value: Box::new(ty_def.value.unwrap_or_else(|| {
                            let mut e = Expr::null();
                            e.ty = Some(Ty {
                                kind: TyKind::Set,
                                name: None,
                            });
                            e
                        })),
                        // FIXME
                        ty_expr: None,
                        kind: VarDefKind::Let,
                    };

                    // This is a hacky way to provide values to std.int and friends.
                    if self.current_module_path == vec![NS_STD] {
                        if let Some(kind) = get_stdlib_decl(&ident.name) {
                            var_def.value.kind = kind;
                        }
                    }

                    self.fold_var_def(var_def)?
                }
                StmtKind::ModuleDef(module_def) => {
                    self.current_module_path.push(ident.name);

                    let decl = Decl {
                        declared_at: stmt.id,
                        kind: DeclKind::Module(Module {
                            names: HashMap::new(),
                            redirects: Vec::new(),
                            shadowed: None,
                        }),
                        order: 0,
                    };
                    let ident = Ident::from_path(self.current_module_path.clone());
                    self.context.root_mod.insert(ident, decl)?;

                    let _ = self.fold_stmts(module_def.stmts)?;
                    self.current_module_path.pop();
                    continue;
                }
            };

            if let VarDefKind::Main = def.kind {
                def.ty_expr = Some(Expr::from(ExprKind::Ident(Ident::from_path(vec![
                    "std", "table",
                ]))));
            }

            let expected_ty = self.fold_type_expr(def.ty_expr)?;
            if expected_ty.is_some() {
                let who = || Some(stmt.name.clone());
                self.context
                    .validate_type(&mut def.value, &expected_ty, who)?;
            }

            let decl = self.context.prepare_expr_decl(def.value);

            self.context
                .declare(ident, decl, stmt.id)
                .with_span(stmt.span)?;
        }
        Ok(())
    }
}

impl AstFold for Resolver {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        Ok(VarDef {
            value: Box::new(Flattener::fold(self.fold_expr(*var_def.value)?)),
            ty_expr: var_def.ty_expr,
            kind: var_def.kind,
        })
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if node.id.is_some() && !matches!(node.kind, ExprKind::Closure(_)) {
            return Ok(node);
        }

        let id = self.id.gen();
        let alias = node.alias.clone();
        let span = node.span;

        if let Some(span) = span {
            self.context.span_map.insert(id, span);
        }

        log::trace!("folding expr {node:?}");

        let mut r = match node.kind {
            ExprKind::Ident(ident) => {
                log::debug!("resolving ident {ident}...");
                let fq_ident = self.resolve_ident(&ident, node.span)?;
                log::debug!("... resolved to {fq_ident}");
                let entry = self.context.root_mod.get(&fq_ident).unwrap();
                log::debug!("... which is {entry}");

                match &entry.kind {
                    DeclKind::Infer(_) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        target_id: entry.declared_at,
                        ..node
                    },
                    DeclKind::Column(target_id) => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        target_id: Some(*target_id),
                        ..node
                    },

                    DeclKind::TableDecl(_) => {
                        let input_name = ident.name.clone();

                        let lineage = self.context.table_decl_to_frame(&fq_ident, input_name, id);

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(Ty {
                                kind: TyKind::Array(Box::new(TyKind::Tuple(
                                    lineage
                                        .columns
                                        .iter()
                                        .map(|col| match col {
                                            LineageColumn::All { .. } => TupleElement::Wildcard,
                                            LineageColumn::Single { name, .. } => {
                                                TupleElement::Single(
                                                    name.as_ref().map(|i| i.name.clone()),
                                                    TyKind::Singleton(Literal::Null),
                                                )
                                            }
                                        })
                                        .collect(),
                                ))),
                                name: None,
                            }),
                            lineage: Some(lineage),
                            alias: None,
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => {
                        if let ExprKind::FuncDef(func_def) = &expr.kind {
                            // TODO: remove this function call entirely when ExprKind::FuncDef is merged with ExprKind::Closure
                            let closure = self.closure_of_func_def(func_def.clone(), fq_ident)?;

                            if self.in_func_call_name {
                                Expr::from(ExprKind::Closure(Box::new(closure)))
                            } else {
                                // this branch is equivalent to the last branch, if ExprKind::Closure is equal to ExprKind::FuncDef
                                self.fold_function(closure, vec![], HashMap::new(), node.span)?
                            }
                        } else {
                            self.fold_expr(expr.as_ref().clone())?
                        }
                    }

                    DeclKind::InstanceOf(_) => {
                        return Err(Error::new_simple(
                            "table instance cannot be referenced directly",
                        )
                        .with_span(span)
                        .with_help("did you forget to specify the column name?")
                        .into());
                    }

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
                self.default_namespace = None;
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                let name = self.fold_expr(*name)?;
                self.in_func_call_name = old;

                let closure = name.try_cast(|n| n.into_closure(), None, "a function")?;

                // fold function
                self.fold_function(*closure, args, named_args, node.span)?
            }

            ExprKind::Pipeline(pipeline) => {
                self.default_namespace = None;
                self.resolve_pipeline(pipeline)?
            }

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

            ExprKind::Unary {
                op: UnOp::Add,
                expr,
            } => self.fold_expr(*expr)?,

            ExprKind::Unary {
                op: UnOp::Not,
                expr,
            } if matches!(expr.kind, ExprKind::List(_)) => self.resolve_column_exclusion(*expr)?,

            ExprKind::All { within, except } => {
                let decl = self.context.root_mod.get(&within);

                // lookup ids of matched inputs
                let target_ids = decl
                    .and_then(|d| d.kind.as_module())
                    .iter()
                    .flat_map(|module| module.as_decls())
                    .sorted_by_key(|(_, decl)| decl.order)
                    .flat_map(|(_, decl)| match &decl.kind {
                        DeclKind::Column(target_id) => Some(*target_id),
                        DeclKind::Infer(_) => decl.declared_at,
                        _ => None,
                    })
                    .unique()
                    .collect();

                let kind = ExprKind::All { within, except };
                Expr {
                    kind,
                    target_ids,
                    ..node
                }
            }

            ExprKind::List(exprs) => {
                let exprs = self.fold_exprs(exprs)?;

                // flatten
                let exprs = exprs
                    .into_iter()
                    .flat_map(|e| match e.kind {
                        ExprKind::List(items) if e.flatten => items,
                        _ => vec![e],
                    })
                    .collect_vec();

                Expr {
                    kind: ExprKind::List(exprs),
                    ..node
                }
            }

            ExprKind::Array(exprs) => {
                let exprs = self.fold_exprs(exprs)?;

                // validate that all elements have the same type
                let mut item_ty = None;
                for expr in &exprs {
                    let ty = &expr.ty;
                    if let Some(item_ty) = item_ty {
                        if item_ty != ty {
                            return Err(Error::new(Reason::Expected {
                                who: Some("array".to_string()),
                                expected: format!(
                                    "types of all of its elements to be {}",
                                    display_ty(item_ty)
                                ),
                                found: display_ty(ty),
                            })
                            .with_span(expr.span)
                            .into());
                        }
                    } else {
                        item_ty = Some(ty);
                    }
                }

                Expr {
                    kind: ExprKind::Array(exprs),
                    ..node
                }
            }

            item => Expr {
                kind: fold_expr_kind(self, item)?,
                ..node
            },
        };
        r.id = r.id.or(Some(id));
        r.alias = r.alias.or(alias);
        r.span = r.span.or(span);

        if r.ty.is_none() {
            r.ty = infer_type(&r)?;
        }
        if r.lineage.is_none() {
            if let ExprKind::TransformCall(call) = &r.kind {
                r.lineage = Some(call.infer_type(&self.context)?);
            }
        }
        if let Some(frame) = &mut r.lineage {
            if let Some(alias) = r.alias.take() {
                frame.rename(alias);
            }
        }
        let r = static_analysis::static_analysis(r);
        Ok(r)
    }
}

impl Resolver {
    fn resolve_pipeline(&mut self, Pipeline { mut exprs }: Pipeline) -> Result<Expr> {
        let mut value = exprs.remove(0);
        value = self.fold_expr(value)?;

        // This is a workaround for pipelines that start with a transform.
        // It checks if first value has resolved to a closure, and if it has,
        // constructs an adhoc closure around the pipeline.
        // Maybe this should not even be supported, or maybe we should have
        // some kind of indication that first element of a pipeline is not a
        // plain value.
        let closure_param = if let ExprKind::Closure(closure) = &mut value.kind {
            // only apply this workaround if closure expects a single arg
            if (closure.params.len() - closure.args.len()) == 1 {
                let param = "_pip_val";
                let value = Expr::from(ExprKind::Ident(Ident::from_name(param)));
                closure.args.push(value);
                Some(param)
            } else {
                None
            }
        } else {
            None
        };

        // the beef of this function: wrapping into func calls
        for expr in exprs {
            let span = expr.span;

            value = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(expr),
                args: vec![value],
                named_args: HashMap::new(),
            }));
            value.span = span;
        }

        // second part of the workaround
        if let Some(closure_param) = closure_param {
            value = Expr::from(ExprKind::Closure(Box::new(Closure {
                name: None,
                body: Box::new(value),
                body_ty: None,

                args: vec![],
                params: vec![ClosureParam {
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
        let res = self
            .context
            .resolve_ident(ident, self.default_namespace.as_ref());

        res.map_err(|e| {
            log::debug!("cannot resolve: `{e}`, context={:#?}", self.context);
            anyhow!(Error::new_simple(e).with_span(span))
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

        let mut r = if enough_args {
            // push the env
            let closure_env = Module::from_exprs(closure.env);
            self.context.root_mod.stack_push(NS_PARAM, closure_env);
            let closure = Closure {
                env: HashMap::new(),
                ..closure
            };

            if log::log_enabled!(log::Level::Debug) {
                let name = closure
                    .name
                    .clone()
                    .unwrap_or_else(|| Ident::from_name("<unnamed>"));
                log::debug!("resolving args of function {}", name);
            }
            let closure = self.resolve_function_args(closure)?;

            // evaluate
            let needs_window = (closure.body_ty)
                .as_ref()
                .map(|ty| ty.is_array())
                .unwrap_or_default();

            let mut res = match self.cast_built_in_function(closure)? {
                // this function call is a built-in function
                Ok(transform) => transform,

                // this function call is not a built-in, proceed with materialization
                Err(closure) => {
                    log::debug!("stack_push for {}", closure.as_debug_name());

                    let (func_env, body) = env_of_closure(closure);

                    self.context.root_mod.stack_push(NS_PARAM, func_env);

                    // fold again, to resolve inner variables & functions
                    let body = self.fold_expr(body)?;

                    // remove param decls
                    log::debug!("stack_pop: {:?}", body.id);
                    let func_env = self.context.root_mod.stack_pop(NS_PARAM).unwrap();

                    if let ExprKind::Closure(mut inner_closure) = body.kind {
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
                    }
                }
            };

            // pop the env
            self.context.root_mod.stack_pop(NS_PARAM).unwrap();

            res.needs_window = needs_window;
            res
        } else {
            // not enough arguments: don't fold
            log::debug!("returning as closure");

            let mut ty = type_of_closure(&closure);
            ty.args = ty.args[args_len..].to_vec();

            let mut node = Expr::from(ExprKind::Closure(Box::new(closure)));
            node.ty = Some(Ty {
                kind: TyKind::Function(ty),
                name: None,
            });

            node
        };
        r.span = span;
        Ok(r)
    }

    fn cast_built_in_function(&mut self, closure: Closure) -> Result<Result<Expr, Closure>> {
        let is_std = closure.name.as_ref().map(|n| n.path.as_slice() == ["std"]);

        if !is_std.unwrap_or_default() {
            return Ok(Err(closure));
        }

        Ok(match transforms::cast_transform(self, closure)? {
            // it a transform
            Ok(e) => Ok(self.fold_expr(e)?),

            // it a std function that should be lowered into a BuiltIn
            Err(closure) if matches!(closure.body.kind, ExprKind::Literal(Literal::Null)) => {
                let name = closure.name.unwrap().to_string();
                let args = closure.args;

                Ok(Expr::from(ExprKind::BuiltInFunction { name, args }))
            }

            // it a normal function that should be resolved
            Err(closure) => Err(closure),
        })
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
                let is_table = param.ty.as_ref().map(|t| t.is_table()).unwrap_or_default();

                is_table
            });

        let has_tables = !tables.is_empty();

        // resolve tables
        if has_tables {
            self.context.root_mod.shadow(NS_FRAME);
            self.context.root_mod.shadow(NS_FRAME_RIGHT);

            for pos in tables.into_iter().with_position() {
                let is_last = matches!(pos, Position::Last(_) | Position::Only(_));
                let (index, (param, arg)) = pos.into_inner();

                // just fold the argument alone
                let mut arg = self.fold_and_type_check(arg, param, func_name)?;
                log::debug!("resolved arg to {}", arg.kind.as_ref());

                // add table's frame into scope
                let frame = arg.lineage.get_or_insert_with(Lineage::default);
                if is_last {
                    self.context.root_mod.insert_frame(frame, NS_FRAME);
                } else {
                    self.context.root_mod.insert_frame(frame, NS_FRAME_RIGHT);
                }

                closure.args[index] = arg;
            }
        }

        // resolve other positional
        for (index, (param, mut arg)) in other {
            if let ExprKind::List(items) = arg.kind {
                // if this is a list, resolve elements separately,
                // so they can be added to scope, before resolving subsequent elements.

                let mut res = Vec::with_capacity(items.len());
                for item in items {
                    let item = self.fold_and_type_check(item, param, func_name)?;

                    // add aliased columns into scope
                    if let Some(alias) = item.alias.clone() {
                        let id = item.id.unwrap();
                        self.context.root_mod.insert_frame_col(NS_FRAME, alias, id);
                    }
                    res.push(item);
                }

                // note that this list node has to be resolved itself
                // (it's elements are already resolved and so their resolving
                // should be skipped)
                arg.kind = ExprKind::List(res);
            }

            let arg = self.fold_and_type_check(arg, param, func_name)?;

            closure.args[index] = arg;
        }

        if has_tables {
            self.context.root_mod.unshadow(NS_FRAME);
            self.context.root_mod.unshadow(NS_FRAME_RIGHT);
        }

        Ok(closure)
    }

    fn fold_and_type_check(
        &mut self,
        arg: Expr,
        param: &ClosureParam,
        func_name: &Option<Ident>,
    ) -> Result<Expr> {
        let mut arg = self.fold_within_namespace(arg, &param.name)?;

        // don't validate types of unresolved exprs
        if arg.id.is_some() {
            // validate type

            let who = || {
                func_name
                    .as_ref()
                    .map(|n| format!("function {n}, param `{}`", param.name))
            };
            self.context.validate_type(&mut arg, &param.ty, who)?;
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

    fn resolve_column_exclusion(&mut self, expr: Expr) -> Result<Expr, anyhow::Error> {
        let expr = self.fold_expr(expr)?;
        let list = coerce_and_flatten(expr)?;
        let except: Vec<Expr> = list
            .into_iter()
            .map(|e| match e.kind {
                ExprKind::Ident(_) | ExprKind::All { .. } => Ok(e),
                _ => Err(Error::new(Reason::Expected {
                    who: Some("exclusion".to_string()),
                    expected: "column name".to_string(),
                    found: format!("`{e}`"),
                })),
            })
            .try_collect()?;

        self.fold_expr(Expr::from(ExprKind::All {
            within: Ident::from_name(NS_FRAME),
            except,
        }))
    }

    fn closure_of_func_def(&mut self, func_def: FuncDef_, fq_ident: Ident) -> Result<Closure> {
        let body_ty = self.fold_type_expr(func_def.return_ty.clone().map(|x| *x))?;

        Ok(Closure {
            name: Some(fq_ident),
            body: func_def.body,
            body_ty,

            params: self.fold_func_params(&func_def.positional_params)?,
            named_params: self.fold_func_params(&func_def.named_params)?,

            args: vec![],

            env: HashMap::default(),
        })
    }

    fn fold_func_params(&mut self, func_params: &[FuncParam]) -> Result<Vec<ClosureParam>> {
        let mut params = Vec::with_capacity(func_params.len());
        for p in func_params.iter().cloned() {
            params.push(ClosureParam {
                name: p.name,
                ty: self.fold_type_expr(p.ty_expr)?,
                default_value: p.default_value,
            })
        }
        Ok(params)
    }

    fn fold_type_expr(&mut self, expr: Option<Expr>) -> Result<Option<Ty>> {
        Ok(match expr {
            Some(expr) => {
                let name = expr.kind.as_ident().map(|i| i.name.clone());

                let expr = self.fold_expr(expr)?;

                let mut set_expr = type_resolver::coerce_to_type(expr, &self.context)?;
                set_expr.name = set_expr.name.or(name);
                Some(set_expr)
            }
            None => None,
        })
    }
}

fn env_of_closure(closure: Closure) -> (Module, Expr) {
    let mut func_env = Module::default();

    for (param, arg) in zip(closure.params, closure.args) {
        let v = Decl {
            declared_at: arg.id,
            kind: DeclKind::Expr(Box::new(arg)),
            order: 0,
        };
        let param_name = param.name.split('.').last().unwrap();
        func_env.names.insert(param_name.to_string(), v);
    }

    (func_env, *closure.body)
}

fn get_stdlib_decl(name: &str) -> Option<ExprKind> {
    let ty_lit = match name {
        "int" => TyLit::Int,
        "float" => TyLit::Float,
        "bool" => TyLit::Bool,
        "text" => TyLit::Text,
        "date" => TyLit::Date,
        "time" => TyLit::Time,
        "timestamp" => TyLit::Timestamp,
        _ => return None,
    };
    Some(ExprKind::Type(TyKind::Primitive(ty_lit)))
}

#[cfg(test)]
mod test {
    use anyhow::Result;
    use insta::assert_yaml_snapshot;

    use crate::ast::pl::{Expr, Lineage};
    use crate::semantic::resolve_only;

    fn parse_and_resolve(query: &str) -> Result<Expr> {
        let ctx = resolve_only(crate::parser::parse(query)?, None)?;
        let (main, _) = ctx.find_main(&[]).unwrap();
        Ok(main.clone())
    }

    fn resolve_lineage(query: &str) -> Result<Lineage> {
        Ok(parse_and_resolve(query)?.lineage.unwrap())
    }

    fn resolve_derive(query: &str) -> Result<Vec<Expr>> {
        let expr = parse_and_resolve(query)?;
        let derive = expr.kind.into_transform_call()?;
        Ok(derive.kind.into_derive()?)
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
            let subtract = a b -> a - b

            from employees
            derive [
                net_salary = subtract gross_salary tax
            ]
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_functions_nested() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            let lag_day = x -> s"lag_day_todo({x})"
            let ret = x dividend_return ->  x / (lag_day x) - 1 + dividend_return

            from a
            derive (ret b c)
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
            let plus_one = x -> x + 1
            let plus = x y -> x + y

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
            let add = x to:1 -> x + to

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
        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from orders
            select [customer_no, gross, tax, gross - tax]
            take 20
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from table_1
            join customers [==customer_no]
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from e = employees
            join salaries [==emp_no]
            group [e.emp_no, e.gender] (
                aggregate [
                    emp_salary = average salaries.salary
                ]
            )
            "#
        )
        .unwrap());
    }
}
