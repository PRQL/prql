use std::collections::HashMap;
use std::iter::zip;

use anyhow::Result;
use itertools::{Itertools, Position};

use crate::ir::pl::*;
use crate::semantic::decl::TableDecl;
use crate::semantic::{static_analysis, NS_PARAM};
use crate::utils::IdGenerator;
use crate::{Error, Reason, Span, WithErrorInfo};

use super::decl::{Decl, DeclKind, TableExpr};
use super::module::Module;
use super::{write_pl, RootModule, NS_DEFAULT_DB, NS_INFER, NS_STD, NS_THAT, NS_THIS};
use flatten::Flattener;
use transforms::coerce_into_tuple_and_flatten;
use type_resolver::infer_type;

mod flatten;
mod root_module_impl;
mod transforms;
mod type_resolver;

/// Can fold (walk) over AST and for each function call or variable find what they are referencing.
pub struct Resolver<'a> {
    context: &'a mut RootModule,

    current_module_path: Vec<String>,

    default_namespace: Option<String>,

    /// Sometimes ident closures must be resolved and sometimes not. See [test::test_func_call_resolve].
    in_func_call_name: bool,

    disable_type_checking: bool,

    pub id: IdGenerator<usize>,

    pub options: ResolverOptions,
}

#[derive(Default, Clone)]
pub struct ResolverOptions {
    pub allow_module_decls: bool,
}

impl Resolver<'_> {
    pub fn new(context: &mut RootModule, options: ResolverOptions) -> Resolver {
        Resolver {
            context,
            options,
            current_module_path: Vec::new(),
            default_namespace: None,
            in_func_call_name: false,
            disable_type_checking: false,
            id: IdGenerator::new(),
        }
    }

    pub fn resolve(&mut self, path: Vec<String>, stmts: Vec<Stmt>) -> Result<()> {
        self.current_module_path = path;
        self.fold_statements(stmts)
    }

    fn fold_statements(&mut self, stmts: Vec<Stmt>) -> Result<()> {
        for mut stmt in stmts {
            stmt.id = Some(self.id.gen());
            if let Some(span) = stmt.span {
                self.context.span_map.insert(stmt.id.unwrap(), span);
            }

            let ident = Ident {
                path: self.current_module_path.clone(),
                name: stmt.name().to_string(),
            };

            let stmt_name = stmt.name().to_string();

            let mut def = match stmt.kind {
                StmtKind::QueryDef(d) => {
                    let decl = DeclKind::QueryDef(*d);
                    self.context
                        .declare(ident, decl, stmt.id, Vec::new())
                        .with_span(stmt.span)?;
                    continue;
                }
                StmtKind::VarDef(var_def) => self.fold_var_def(var_def)?,
                StmtKind::TypeDef(ty_def) => {
                    let mut value = if let Some(value) = ty_def.value {
                        value
                    } else {
                        Box::new(Expr::new(Literal::Null))
                    };

                    // This is a hacky way to provide values to std.int and friends.
                    if self.current_module_path == vec![NS_STD] {
                        if let Some(kind) = get_stdlib_decl(&ident.name) {
                            value.kind = kind;
                        }
                    }

                    let mut ty = self.fold_type_expr(Some(value))?.unwrap();
                    ty.name = Some(ident.name.clone());

                    VarDef {
                        name: ty_def.name,
                        value: Box::new(Expr::new(ExprKind::Type(ty))),
                        ty_expr: None,
                    }
                }
                StmtKind::ModuleDef(module_def) => {
                    if !self.options.allow_module_decls {
                        return Err(Error::new_simple(
                            "explicit module declarations are not allowed",
                        )
                        .with_span(stmt.span)
                        .into());
                    }

                    self.current_module_path.push(ident.name);

                    let decl = Decl {
                        declared_at: stmt.id,
                        kind: DeclKind::Module(Module {
                            names: HashMap::new(),
                            redirects: Vec::new(),
                            shadowed: None,
                        }),
                        ..Default::default()
                    };
                    let ident = Ident::from_path(self.current_module_path.clone());
                    self.context
                        .root_mod
                        .insert(ident, decl)
                        .with_span(stmt.span)?;

                    self.fold_statements(module_def.stmts)?;
                    self.current_module_path.pop();
                    continue;
                }
            };

            if def.name == "main" {
                def.ty_expr = Some(Box::new(Expr::new(ExprKind::Ident(Ident::from_path(
                    vec!["std", "relation"],
                )))));
            }

            if let ExprKind::Func(closure) = &mut def.value.kind {
                if closure.name_hint.is_none() {
                    closure.name_hint = Some(ident.clone());
                }
            }

            let expected_ty = self.fold_type_expr(def.ty_expr)?;
            if expected_ty.is_some() {
                let who = || Some(stmt_name.clone());
                self.validate_type(&mut def.value, expected_ty.as_ref(), &who)?;
            }

            let decl = self.context.prepare_expr_decl(def.value);

            self.context
                .declare(ident, decl, stmt.id, stmt.annotations)
                .with_span(stmt.span)?;
        }
        Ok(())
    }

    /// Converts a identifier that points to a table declaration to a frame of that table.
    fn lineage_of_table_decl(
        &mut self,
        table_fq: &Ident,
        input_name: String,
        input_id: usize,
    ) -> Lineage {
        let id = input_id;
        let table_decl = self.context.root_mod.get(table_fq).unwrap();
        let TableDecl { ty, .. } = table_decl.kind.as_table_decl().unwrap();

        // TODO: can this panic?
        let columns = ty.as_ref().unwrap().as_relation().unwrap();

        let mut instance_frame = Lineage {
            inputs: vec![LineageInput {
                id,
                name: input_name.clone(),
                table: table_fq.clone(),
            }],
            columns: Vec::new(),
            ..Default::default()
        };

        for col in columns {
            let col = match col {
                TupleField::Wildcard(_) => LineageColumn::All {
                    input_name: input_name.clone(),
                    except: columns
                        .iter()
                        .flat_map(|c| c.as_single().map(|x| x.0).cloned().flatten())
                        .collect(),
                },
                TupleField::Single(col_name, _) => LineageColumn::Single {
                    name: col_name
                        .clone()
                        .map(|col_name| Ident::from_path(vec![input_name.clone(), col_name])),
                    target_id: id,
                    target_name: col_name.clone(),
                },
            };
            instance_frame.columns.push(col);
        }

        log::debug!("instanced table {table_fq} as {instance_frame:?}");
        instance_frame
    }

    /// Declares a new table for a relation literal.
    /// This is needed for column inference to work properly.
    fn declare_table_for_literal(
        &mut self,
        input_id: usize,
        columns: Option<Vec<TupleField>>,
        name_hint: Option<String>,
    ) -> Lineage {
        let id = input_id;
        let global_name = format!("_literal_{}", id);

        // declare a new table in the `default_db` module
        let default_db_ident = Ident::from_name(NS_DEFAULT_DB);
        let default_db = self.context.root_mod.get_mut(&default_db_ident).unwrap();
        let default_db = default_db.kind.as_module_mut().unwrap();

        let infer_default = default_db.get(&Ident::from_name(NS_INFER)).unwrap().clone();
        let mut infer_default = *infer_default.kind.into_infer().unwrap();

        let table_decl = infer_default.as_table_decl_mut().unwrap();
        table_decl.expr = TableExpr::None;

        if let Some(columns) = columns {
            table_decl.ty = Some(Ty::relation(columns));
        }

        default_db
            .names
            .insert(global_name.clone(), Decl::from(infer_default));

        // produce a frame of that table
        let input_name = name_hint.unwrap_or_else(|| global_name.clone());
        let table_fq = default_db_ident + Ident::from_name(global_name);
        self.lineage_of_table_decl(&table_fq, input_name, id)
    }
}

impl PlFold for Resolver<'_> {
    fn fold_stmts(&mut self, _: Vec<Stmt>) -> Result<Vec<Stmt>> {
        unreachable!()
    }

    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        let value = if matches!(var_def.value.kind, ExprKind::Func(_)) {
            var_def.value
        } else {
            Box::new(Flattener::fold(self.fold_expr(*var_def.value)?))
        };

        Ok(VarDef {
            name: var_def.name,
            value,
            ty_expr: fold_optional_box(self, var_def.ty_expr)?,
        })
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if node.id.is_some() && !matches!(node.kind, ExprKind::Func(_)) {
            return Ok(node);
        }

        let id = self.id.gen();
        let alias = node.alias.clone();
        let span = node.span;

        if let Some(span) = span {
            self.context.span_map.insert(id, span);
        }

        log::trace!("folding expr {node:?}");

        let r = match node.kind {
            ExprKind::Ident(ident) => {
                log::debug!("resolving ident {ident}...");
                let fq_ident = self.resolve_ident(&ident).with_span(node.span)?;
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

                        let lineage = self.lineage_of_table_decl(&fq_ident, input_name, id);

                        Expr {
                            kind: ExprKind::Ident(fq_ident),
                            ty: Some(ty_of_lineage(&lineage)),
                            lineage: Some(lineage),
                            alias: None,
                            ..node
                        }
                    }

                    DeclKind::Expr(expr) => match &expr.kind {
                        ExprKind::Func(closure) => {
                            let closure = self.fold_function_types(*closure.clone())?;

                            let expr = Expr::new(ExprKind::Func(Box::new(closure)));

                            if self.in_func_call_name {
                                expr
                            } else {
                                self.fold_expr(expr)?
                            }
                        }
                        _ => self.fold_expr(expr.as_ref().clone())?,
                    },

                    DeclKind::InstanceOf(_) => {
                        return Err(Error::new_simple(
                            "table instance cannot be referenced directly",
                        )
                        .with_span(span)
                        .push_hint("did you forget to specify the column name?")
                        .into());
                    }

                    _ => Expr {
                        kind: ExprKind::Ident(fq_ident),
                        ..node
                    },
                }
            }

            ExprKind::FuncCall(FuncCall { name, args, .. })
                if (name.kind.as_ident()).map_or(false, |i| i.to_string() == "std.not")
                    && matches!(args[0].kind, ExprKind::Tuple(_)) =>
            {
                let arg = args.into_iter().exactly_one().unwrap();
                self.resolve_column_exclusion(arg)?
            }

            ExprKind::FuncCall(FuncCall {
                name,
                args,
                named_args,
            }) => {
                // fold function name
                self.default_namespace = None;
                let old = self.in_func_call_name;
                self.in_func_call_name = true;
                let name = self.fold_expr(*name)?;
                self.in_func_call_name = old;

                let func = *name.try_cast(|n| n.into_func(), None, "a function")?;

                // fold function
                let func = self.apply_args_to_closure(func, args, named_args)?;
                self.fold_function(func, span)?
            }

            ExprKind::Func(closure) => self.fold_function(*closure, span)?,

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

            ExprKind::Tuple(exprs) => {
                let exprs = self.fold_exprs(exprs)?;

                // flatten
                let exprs = exprs
                    .into_iter()
                    .flat_map(|e| match e.kind {
                        ExprKind::Tuple(items) if e.flatten => items,
                        _ => vec![e],
                    })
                    .collect_vec();

                Expr {
                    kind: ExprKind::Tuple(exprs),
                    ..node
                }
            }

            ExprKind::Array(exprs) => {
                let mut exprs = self.fold_exprs(exprs)?;

                // validate that all elements have the same type
                let mut expected_ty: Option<&Ty> = None;
                for expr in &mut exprs {
                    if expr.ty.is_some() {
                        if expected_ty.is_some() {
                            let who = || Some("array".to_string());
                            self.validate_type(expr, expected_ty, &who)?;
                        }
                        expected_ty = expr.ty.as_ref();
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
        let mut r = static_analysis::static_analysis(r);
        r.id = r.id.or(Some(id));
        r.alias = r.alias.or(alias);
        r.span = r.span.or(span);

        if r.ty.is_none() {
            r.ty = infer_type(&r)?;
        }
        if r.lineage.is_none() {
            if let ExprKind::TransformCall(call) = &r.kind {
                r.lineage = Some(call.infer_type(self.context)?);
            } else if let Some(relation_columns) = r.ty.as_ref().and_then(|t| t.as_relation()) {
                // lineage from ty

                let columns = Some(relation_columns.clone());

                let name = r.alias.clone();
                let frame = self.declare_table_for_literal(id, columns, name);

                r.lineage = Some(frame);
            }
        }
        if let Some(lineage) = &mut r.lineage {
            if let Some(alias) = r.alias.take() {
                lineage.rename(alias.clone());

                if let Some(ty) = &mut r.ty {
                    ty.kind.rename_relation(alias);
                }
            }
        }
        Ok(r)
    }
}

impl Resolver<'_> {
    pub fn resolve_ident(&mut self, ident: &Ident) -> Result<Ident, Error> {
        if let Some(default_namespace) = &self.default_namespace {
            self.context.resolve_ident(ident, Some(default_namespace))
        } else {
            let mut ident = ident.clone().prepend(self.current_module_path.clone());

            let mut res = self.context.resolve_ident(&ident, None);
            for _ in &self.current_module_path {
                if res.is_ok() {
                    break;
                }
                ident = ident.pop_front().1.unwrap();
                res = self.context.resolve_ident(&ident, None);
            }
            res
        }

        // log::debug!(
        //     "cannot resolve `{ident}`: `{e}`, context={:#?}",
        //     self.context
        // );
    }

    fn fold_function(&mut self, closure: Func, span: Option<Span>) -> Result<Expr> {
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
        self.context.root_mod.stack_push(NS_PARAM, closure_env);
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
            .map(|t| t.as_ty().unwrap().is_sub_type_of_array())
            .unwrap_or_default();

        // evaluate
        let res = if let ExprKind::Internal(operator_name) = &closure.body.kind {
            // special case: functions that have internal body

            if operator_name.starts_with("std.") {
                Expr {
                    ty: closure.return_ty.map(|t| t.into_ty().unwrap()),
                    needs_window,
                    ..Expr::new(ExprKind::RqOperator {
                        name: operator_name.clone(),
                        args: closure.args,
                    })
                }
            } else {
                let expr = transforms::cast_transform(self, closure)?;
                self.fold_expr(expr)?
            }
        } else {
            // base case: materialize
            log::debug!("stack_push for {}", closure.as_debug_name());

            let (func_env, body) = env_of_closure(closure);

            self.context.root_mod.stack_push(NS_PARAM, func_env);

            // fold again, to resolve inner variables & functions
            let body = self.fold_expr(body)?;

            // remove param decls
            log::debug!("stack_pop: {:?}", body.id);
            let func_env = self.context.root_mod.stack_pop(NS_PARAM).unwrap();

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
        self.context.root_mod.stack_pop(NS_PARAM).unwrap();

        Ok(Expr { span, ..res })
    }

    fn fold_function_types(&mut self, mut closure: Func) -> Result<Func> {
        closure.params = closure
            .params
            .into_iter()
            .map(|p| -> Result<_> {
                Ok(FuncParam {
                    ty: self.fold_ty_or_expr(p.ty)?,
                    ..p
                })
            })
            .try_collect()?;
        closure.return_ty = self.fold_ty_or_expr(closure.return_ty)?;
        Ok(closure)
    }

    fn apply_args_to_closure(
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
                    .and_then(|t| t.as_ty())
                    .map(|t| t.is_relation())
                    .unwrap_or_default();

                is_relation
            });

        let has_relations = !relations.is_empty();

        // resolve relational args
        if has_relations {
            self.context.root_mod.shadow(NS_THIS);
            self.context.root_mod.shadow(NS_THAT);

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
                        self.context.root_mod.insert_frame(frame, NS_THIS);
                    } else {
                        self.context.root_mod.insert_frame(frame, NS_THAT);
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
                            self.context.root_mod.insert_frame_col(NS_THIS, alias, id);
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
            self.context.root_mod.unshadow(NS_THIS);
            self.context.root_mod.unshadow(NS_THAT);
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
        if arg.id.is_some() && !self.disable_type_checking {
            // validate type

            let expects_func = param
                .ty
                .as_ref()
                .map(|t| t.as_ty().unwrap().is_function())
                .unwrap_or_default();
            if !expects_func && arg.kind.is_func() {
                return Ok(Err(arg));
            }

            let who = || {
                func_name
                    .as_ref()
                    .map(|n| format!("function {n}, param `{}`", param.name))
            };
            let ty = param.ty.as_ref().map(|t| t.as_ty().unwrap());
            self.validate_type(&mut arg, ty, &who)?;
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

    fn resolve_column_exclusion(&mut self, expr: Expr) -> Result<Expr> {
        let expr = self.fold_expr(expr)?;
        let tuple = coerce_into_tuple_and_flatten(expr)?;
        let except: Vec<Expr> = tuple
            .into_iter()
            .map(|e| match e.kind {
                ExprKind::Ident(_) | ExprKind::All { .. } => Ok(e),
                _ => Err(Error::new(Reason::Expected {
                    who: Some("exclusion".to_string()),
                    expected: "column name".to_string(),
                    found: format!("`{}`", write_pl(e)),
                })),
            })
            .try_collect()?;

        self.fold_expr(Expr::new(ExprKind::All {
            within: Ident::from_name(NS_THIS),
            except,
        }))
    }

    pub fn fold_type_expr(&mut self, expr: Option<Box<Expr>>) -> Result<Option<Ty>> {
        Ok(match expr {
            Some(expr) => {
                let name = expr.kind.as_ident().map(|i| i.name.clone());

                let old = self.disable_type_checking;
                self.disable_type_checking = true;
                let expr = self.fold_expr(*expr)?;
                self.disable_type_checking = old;

                let mut set_expr = type_resolver::coerce_to_type(self, expr)?;
                set_expr.name = set_expr.name.or(name);
                Some(set_expr)
            }
            None => None,
        })
    }

    fn fold_ty_or_expr(&mut self, ty_or_expr: Option<TyOrExpr>) -> Result<Option<TyOrExpr>> {
        self.context.root_mod.shadow(NS_THIS);
        self.context.root_mod.shadow(NS_THAT);

        let res = match ty_or_expr {
            Some(TyOrExpr::Expr(ty_expr)) => {
                Some(TyOrExpr::Ty(self.fold_type_expr(Some(ty_expr))?.unwrap()))
            }
            _ => ty_or_expr,
        };

        self.context.root_mod.unshadow(NS_THIS);
        self.context.root_mod.unshadow(NS_THAT);
        Ok(res)
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

fn expr_of_func(func: Func, span: Option<Span>) -> Expr {
    let ty = TyFunc {
        args: func
            .params
            .iter()
            .skip(func.args.len())
            .map(|a| a.ty.as_ref().map(|x| x.as_ty().cloned().unwrap()))
            .collect(),
        return_ty: Box::new(func.return_ty.as_ref().map(|x| x.as_ty().cloned().unwrap())),
    };

    Expr {
        ty: Some(Ty {
            kind: TyKind::Function(Some(ty)),
            name: None,
        }),
        span,
        ..Expr::new(ExprKind::Func(Box::new(func)))
    }
}

fn ty_of_lineage(lineage: &Lineage) -> Ty {
    Ty::relation(
        lineage
            .columns
            .iter()
            .map(|col| match col {
                LineageColumn::All { .. } => TupleField::Wildcard(None),
                LineageColumn::Single { name, .. } => TupleField::Single(
                    name.as_ref().map(|i| i.name.clone()),
                    Some(Ty {
                        kind: TyKind::Singleton(Literal::Null),
                        name: None,
                    }),
                ),
            })
            .collect(),
    )
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

fn get_stdlib_decl(name: &str) -> Option<ExprKind> {
    let set = match name {
        "int" => PrimitiveSet::Int,
        "float" => PrimitiveSet::Float,
        "bool" => PrimitiveSet::Bool,
        "text" => PrimitiveSet::Text,
        "date" => PrimitiveSet::Date,
        "time" => PrimitiveSet::Time,
        "timestamp" => PrimitiveSet::Timestamp,
        "func" => {
            return Some(ExprKind::Type(Ty {
                kind: TyKind::Function(None),
                name: None,
            }))
        }
        "anytype" => {
            return Some(ExprKind::Type(Ty {
                kind: TyKind::Any,
                name: None,
            }))
        }
        _ => return None,
    };
    Some(ExprKind::Type(Ty {
        kind: TyKind::Primitive(set),
        name: None,
    }))
}

#[cfg(test)]
pub(super) mod test {
    use anyhow::Result;
    use insta::assert_yaml_snapshot;

    use crate::ir::pl::{Expr, Lineage, PlFold};

    pub fn erase_ids(expr: Expr) -> Expr {
        IdEraser {}.fold_expr(expr).unwrap()
    }

    struct IdEraser {}

    impl PlFold for IdEraser {
        fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
            expr.kind = self.fold_expr_kind(expr.kind)?;
            expr.id = None;
            expr.target_id = None;
            expr.target_ids.clear();
            Ok(expr)
        }
    }

    fn parse_and_resolve(query: &str) -> Result<Expr> {
        let ctx = crate::semantic::test::parse_and_resolve(query)?;
        let (main, _) = ctx.find_main_rel(&[]).unwrap();
        Ok(*main.clone().into_relation_var().unwrap())
    }

    fn resolve_lineage(query: &str) -> Result<Lineage> {
        Ok(parse_and_resolve(query)?.lineage.unwrap())
    }

    fn resolve_derive(query: &str) -> Result<Vec<Expr>> {
        let expr = parse_and_resolve(query)?;
        let derive = expr.kind.into_transform_call().unwrap();
        let exprs = derive
            .kind
            .into_derive()
            .unwrap_or_else(|e| panic!("Failed to convert `{e:?}`"));

        let exprs = IdEraser {}.fold_exprs(exprs).unwrap();
        Ok(exprs)
    }

    #[test]
    fn test_variables_1() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            from employees
            derive {
                gross_salary = salary + payroll_tax,
                gross_cost =   gross_salary + benefits_cost
            }
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
            derive {
                net_salary = subtract gross_salary tax
            }
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
            derive {b = (sum foo | plus_one | plus 2)}
            "#
        )
        .unwrap());
    }
    #[test]
    fn test_named_args() {
        assert_yaml_snapshot!(resolve_derive(
            r#"
            let add_one = x to:1 -> x + to

            from foo_table
            derive {
                added = add_one bar to:3,
                added_default = add_one bar
            }
            "#
        )
        .unwrap());
    }

    #[test]
    fn test_frames_and_names() {
        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from orders
            select {customer_no, gross, tax, gross - tax}
            take 20
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from table_1
            join customers (==customer_no)
            "#
        )
        .unwrap());

        assert_yaml_snapshot!(resolve_lineage(
            r#"
            from e = employees
            join salaries (==emp_no)
            group {e.emp_no, e.gender} (
                aggregate {
                    emp_salary = average salaries.salary
                }
            )
            "#
        )
        .unwrap());
    }
}
