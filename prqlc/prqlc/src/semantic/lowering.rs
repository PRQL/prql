use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::iter::zip;

use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::ast::generic::{InterpolateItem, Range, SwitchCase};
use crate::ast::TyTupleField;
use crate::ir::decl::{self, DeclKind, Module, RootModule, TableExpr};
use crate::ir::generic::{ColumnSort, WindowFrame};
use crate::ir::pl::{self, Ident, PlFold, QueryDef};
use crate::ir::rq::{self, CId, RelationColumn, RelationalQuery, TId, TableDecl, Transform};
use crate::semantic::write_pl;
use crate::utils::{toposort, IdGenerator};
use crate::COMPILER_VERSION;
use crate::{Error, Reason, Result, Span, WithErrorInfo};

use super::{NS_LOCAL, NS_THAT, NS_THIS};

/// Convert a resolved expression at path `main_path` relative to `root_mod`
/// into RQ and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved expressions.
///
/// All table references must reside within module at `database_module_path`.
/// They are compiled to table identifiers, using their path relative to the database module.
/// For example, with `database_module_path=my_database`:
/// - `my_database.my_table` will compile to `"my_table"`,
/// - `my_database.my_schema.my_table` will compile to `"my_schema.my_table"`,
/// - `my_table` will error out saying that this table does not reside in current database.
pub fn lower_to_ir(
    root_mod: RootModule,
    main_path: &[String],
    database_module_path: &[String],
) -> Result<(RelationalQuery, RootModule)> {
    // find main
    log::debug!("lookup for main pipeline in {main_path:?}");
    let (_, main_ident) = root_mod.find_main_rel(main_path).map_err(|(hint, span)| {
        Error::new_simple("Missing main pipeline")
            .with_code("E0001")
            .with_hints(hint)
            .with_span(span)
    })?;

    // find & validate query def
    let def = root_mod.find_query_def(&main_ident);
    let def = def.cloned().unwrap_or_default();
    validate_query_def(&def)?;

    // find all tables in the root module
    let tables = TableExtractor::extract(&root_mod.module);

    // prune and toposort
    let tables = toposort_tables(tables, &main_ident);

    // lower tables
    let mut l = Lowerer::new(root_mod, database_module_path);
    let mut main_relation = None;
    for (fq_ident, (table, declared_at)) in tables {
        let is_main = fq_ident == main_ident;

        log::debug!("lowering: {:#?}", table.expr.as_relation_var());

        l.lower_table_decl(table, fq_ident)
            .map_err(with_span_if_not_exists(|| get_span_of_id(&l, declared_at)))?;

        if is_main {
            let main_table = l.table_buffer.pop().unwrap();
            main_relation = Some(main_table.relation);
        }
    }

    let query = RelationalQuery {
        def,
        tables: l.table_buffer,
        relation: main_relation.unwrap(),
    };
    Ok((query, l.root_mod))
}

fn extern_ref_to_relation(
    ty_tuple_fields: Vec<TyTupleField>,
    fq_ident: &Ident,
    database_module_path: &[String],
) -> Result<(rq::Relation, Option<String>), Error> {
    let extern_name = if fq_ident.starts_with_path(database_module_path) {
        let relative_to_database: Vec<&String> =
            fq_ident.iter().skip(database_module_path.len()).collect();
        if relative_to_database.is_empty() {
            None
        } else {
            Some(Ident::from_path(relative_to_database))
        }
    } else {
        None
    };

    let Some(extern_name) = extern_name else {
        let database_module = Ident::from_path(database_module_path.to_vec());
        return Err(Error::new_simple("this table is not in the current database")
            .push_hint(format!("If this is a table in the current database, move its declaration into module {database_module}")));
    };

    // put wildcards last
    let mut ty_tuple_fields = ty_tuple_fields;
    ty_tuple_fields.sort_by_key(|a| matches!(a, TyTupleField::Wildcard(_)));

    let relation = rq::Relation {
        kind: rq::RelationKind::ExternRef(extern_name),
        columns: ty_tuple_to_relation_columns(ty_tuple_fields, None),
    };
    Ok((relation, None))
}

fn ty_tuple_to_relation_columns(
    fields: Vec<TyTupleField>,
    prefix: Option<String>,
) -> Vec<RelationColumn> {
    fields
        .into_iter()
        .flat_map(|field| match field {
            TyTupleField::Single(mut name, ty) => {
                if let Some(p) = &prefix {
                    if let Some(n) = &mut name {
                        *n = format!("{p}.{n}");
                    } else {
                        name = Some(p.clone());
                    }
                }

                if ty.as_ref().map_or(false, |t| t.kind.is_tuple()) {
                    let inner = ty.unwrap().kind.into_tuple().unwrap();
                    return ty_tuple_to_relation_columns(inner, name);
                }

                vec![RelationColumn::Single(name)]
            }
            TyTupleField::Wildcard(_) => vec![RelationColumn::Wildcard],
        })
        .collect_vec()
}

fn validate_query_def(query_def: &QueryDef) -> Result<()> {
    if let Some(requirement) = &query_def.version {
        if !requirement.matches(&COMPILER_VERSION) {
            return Err(Error::new_simple("This query uses a version of PRQL that is not supported by prqlc. Please upgrade the compiler."));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Lowerer {
    cid: IdGenerator<CId>,
    tid: IdGenerator<TId>,
    id: IdGenerator<usize>,

    root_mod: RootModule,
    database_module_path: Vec<String>,

    /// describes what has certain id has been lowered to
    node_mapping: HashMap<usize, LoweredTarget>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_mapping: HashMap<Ident, TId>,

    // current window for any new column defs
    window: Option<rq::Window>,

    /// A buffer to be added into current pipeline
    pipeline: Vec<Transform>,

    /// A buffer to be added into query tables
    table_buffer: Vec<TableDecl>,

    local_this_id: Option<usize>,
    local_that_id: Option<usize>,
}

#[derive(Clone, EnumAsInner, Debug)]
enum LoweredTarget {
    /// Lowered node was a computed expression.
    Column(CId),

    /// Lowered node was a tuple with following columns.
    Relation(Vec<usize>),
}

impl Lowerer {
    fn new(root_mod: RootModule, database_module_path: &[String]) -> Self {
        Lowerer {
            root_mod,
            database_module_path: database_module_path.to_vec(),

            cid: IdGenerator::new(),
            tid: IdGenerator::new(),
            id: {
                // HACK: create id generator start starts at really large numbers
                //   because we need to invent new ids after the resolver has finished.
                let mut gen = IdGenerator::new();
                gen.skip(100000000);
                gen
            },

            node_mapping: HashMap::new(),
            table_mapping: HashMap::new(),

            window: None,
            pipeline: Vec::new(),
            table_buffer: Vec::new(),

            local_this_id: None,
            local_that_id: None,
        }
    }

    fn lower_table_decl(&mut self, table: decl::TableDecl, fq_ident: Ident) -> Result<()> {
        let decl::TableDecl { ty, expr } = table;

        // TODO: can this panic?
        let columns = ty.unwrap().into_relation().unwrap();

        let (relation, name) = match expr {
            TableExpr::RelationVar(expr) => {
                // a CTE
                (self.lower_relation(*expr)?, Some(fq_ident.name.clone()))
            }
            TableExpr::LocalTable => {
                extern_ref_to_relation(columns, &fq_ident, &self.database_module_path)?
            }
            TableExpr::Param(_) => unreachable!(),
            TableExpr::None => return Ok(()),
        };

        let id = *self
            .table_mapping
            .entry(fq_ident)
            .or_insert_with(|| self.tid.gen());

        log::debug!("lowering table {name:?}, columns = {:?}", relation.columns);

        let table = TableDecl { id, name, relation };
        self.table_buffer.push(table);
        Ok(())
    }

    /// Lower an expression into a instance of a table in the query
    fn lower_table_ref(&mut self, expr: pl::Expr) -> Result<rq::TableRef> {
        let expr = expr;

        Ok(match expr.kind {
            pl::ExprKind::Ident(fq_table_name) => {
                // ident that refer to table: create an instance of the table
                let id = expr.id.unwrap();
                let tid = *self.table_mapping.get(&fq_table_name).unwrap();

                log::debug!("lowering an instance of table {fq_table_name} (id={id})...");

                self.create_table_instance(id, None, tid)
            }
            pl::ExprKind::TransformCall(_) => {
                // pipeline that has to be pulled out into a table
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                let relation = self.lower_relation(expr)?;

                let last_transform = &relation.kind.as_pipeline().unwrap().last().unwrap();
                let cids = last_transform.as_select().unwrap().clone();

                log::debug!("lowering inline table, columns = {:?}", relation.columns);
                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                let table_ref = self.create_table_instance(id, None, tid);

                let redirects = zip(cids, table_ref.columns.iter().map(|(_, c)| *c)).collect();
                self.redirect_mappings(redirects);

                table_ref
            }
            pl::ExprKind::SString(_items) => {
                // let id = expr.id.unwrap();

                // create a new table
                // let tid = self.tid.gen();

                // pull columns from the table decl
                // TODO: can this panic?

                todo!();

                // lower the expr
                // let items = self.lower_interpolations(items)?;
                // let relation = rq::Relation {
                //     kind: rq::RelationKind::SString(items),
                //     columns: tuple_fields_to_relation_columns(columns),
                // };

                // self.table_buffer.push(TableDecl {
                //     id: tid,
                //     name: None,
                //     relation,
                // });

                // return an instance of this new table
                // self.create_a_table_instance(id, None, tid)
            }
            pl::ExprKind::RqOperator { .. } => {
                todo!();

                // let id = expr.id.unwrap();

                // create a new table
                // let tid = self.tid.gen();

                // lower the expr
                // let args = args.into_iter().map(|a| self.lower_expr(a)).try_collect()?;
                // let relation = rq::Relation {
                //     kind: rq::RelationKind::BuiltInFunction { name, args },
                //     columns: tuple_fields_to_relation_columns(vec![]),
                // };

                // self.table_buffer.push(TableDecl {
                //     id: tid,
                //     name: None,
                //     relation,
                // });

                // // return an instance of this new table
                // self.create_a_table_instance(id, None, tid)
            }

            pl::ExprKind::Array(_) => {
                todo!();

                /*
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                // pull columns from the table decl


                let lit = RelationLiteral {
                    columns: vec![],
                    rows: elements
                        .into_iter()
                        .map(|row| {
                            row.kind
                                .into_tuple()
                                .unwrap()
                                .into_iter()
                                .map(|element| {
                                    element.try_cast(
                                        |x| x.into_literal(),
                                        Some("relation literal"),
                                        "literals",
                                    )
                                })
                                .try_collect()
                        })
                        .try_collect()?,
                };

                let columns = vec![];
                log::debug!("lowering literal relation table, columns = {columns:?}");
                let relation = rq::Relation {
                    kind: rq::RelationKind::Literal(lit),
                    columns,
                };

                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                self.create_a_table_instance(id, None, tid)
                 */
            }

            _ => {
                return Err(Error::new(Reason::Expected {
                    who: None,
                    expected: "a pipeline that resolves to a table".to_string(),
                    found: format!("`{}`", write_pl(expr.clone())),
                })
                .push_hint("are you missing `from` statement?")
                .with_span(expr.span))
            }
        })
    }

    fn redirect_mappings(&mut self, redirects: HashMap<CId, CId>) {
        for target in self.node_mapping.values_mut() {
            match target {
                LoweredTarget::Column(cid) => {
                    if let Some(new) = redirects.get(cid) {
                        *cid = *new;
                    }
                }
                LoweredTarget::Relation(_) => todo!(),
            }
        }
    }

    fn create_table_instance(&mut self, id: usize, name: Option<String>, tid: TId) -> rq::TableRef {
        // create instance columns from table columns
        let table = self.table_buffer.iter().find(|t| t.id == tid).unwrap();

        let columns = (table.relation.columns.iter())
            .cloned()
            .unique()
            .map(|col| (col, self.cid.gen()))
            .collect_vec();

        log::debug!("... columns = {:?}", columns);

        let mut rel_columns = Vec::new();
        for (_, cid) in &columns {
            let id = self.id.gen();
            self.node_mapping.insert(id, LoweredTarget::Column(*cid));

            rel_columns.push(id);
        }
        self.node_mapping
            .insert(id, LoweredTarget::Relation(rel_columns));

        rq::TableRef {
            source: tid,
            name,
            columns,
        }
    }

    fn lower_relation(&mut self, expr: pl::Expr) -> Result<rq::Relation> {
        let id = expr.id.unwrap();

        let relation_fields = expr.ty.as_ref().and_then(|t| t.as_relation()).unwrap();
        let columns = ty_tuple_to_relation_columns(relation_fields.clone(), None);

        let prev_pipeline = self.pipeline.drain(..).collect_vec();

        self.lower_relational_expr(expr, None)?;

        let mut transforms = self.pipeline.drain(..).collect_vec();
        self.pipeline = prev_pipeline;

        transforms.push(Transform::Select(
            self.flatten_tuple_fields_into_cids(&[id]),
        ));
        let relation = rq::Relation {
            kind: rq::RelationKind::Pipeline(transforms),
            columns,
        };
        Ok(relation)
    }

    /// HACK: Result is stored in self.pipeline
    fn lower_relational_expr(&mut self, ast: pl::Expr, closure_param: Option<usize>) -> Result<()> {
        // find the actual transform that we want to compile to relational pipeline
        // this is non trivial, because sometimes the transforms will be wrapped into
        // functions that are still waiting for arguments
        // for example: this would happen when lowering loop's pipeline
        match ast.kind {
            // base case
            pl::ExprKind::TransformCall(transform) => {
                let tuple_fields = self.lower_transform_call(transform, closure_param, ast.span)?;

                self.node_mapping
                    .insert(ast.id.unwrap(), LoweredTarget::Relation(tuple_fields));
            }

            // actually operate on func's body
            pl::ExprKind::Func(func) => {
                let param = func.params.first();
                let param = param.and_then(|p| p.name.parse::<usize>().ok());
                self.lower_relational_expr(*func.body, param)?;
            }

            // this relational expr is not a transform
            _ => {
                if let Some(target) = ast.target_id {
                    if Some(target) == closure_param {
                        // ast is a closure param, so don't need to push From
                        return Ok(());
                    }
                }

                let table_ref = self.lower_table_ref(ast)?;
                self.pipeline.push(Transform::From(table_ref));
            }
        };
        Ok(())
    }

    /// HACK: Result is stored in self.pipeline
    fn lower_transform_call(
        &mut self,
        transform_call: pl::TransformCall,
        closure_param: Option<usize>,
        span: Option<Span>,
    ) -> Result<Vec<usize>> {
        // lower input table
        let input_id = transform_call.input.id.unwrap();
        self.lower_relational_expr(*transform_call.input, closure_param)?;

        // ... and continues with transforms created in this function
        self.local_this_id = Some(input_id);

        let window = rq::Window {
            frame: WindowFrame {
                kind: transform_call.frame.kind,
                range: self.lower_range(transform_call.frame.range)?,
            },
            partition: if let Some(partition) = transform_call.partition {
                let ids = self.lower_and_flatten_tuple(*partition, false)?;
                self.flatten_tuple_fields_into_cids(&ids)
            } else {
                vec![]
            },
            sort: self.lower_sorts(transform_call.sort)?,
        };
        self.window = Some(window);

        let new_fields: Option<Vec<usize>> = match *transform_call.kind {
            pl::TransformKind::Derive { assigns, .. } => {
                let ids = self.lower_and_flatten_tuple(*assigns, false)?;
                Some(ids)
            }
            pl::TransformKind::Select { assigns, .. } => {
                let ids = self.lower_and_flatten_tuple(*assigns, false)?;
                Some(ids)
            }
            pl::TransformKind::Filter { filter, .. } => {
                let filter = self.lower_expr(*filter)?;

                self.pipeline.push(Transform::Filter(filter));

                None
            }
            pl::TransformKind::Aggregate { assigns, .. } => {
                let window = self.window.take();

                let ids = self.lower_and_flatten_tuple(*assigns, true)?;

                let partition = window.unwrap().partition;
                self.pipeline.push(Transform::Aggregate {
                    partition,
                    compute: self.flatten_tuple_fields_into_cids(&ids),
                });

                Some(ids)
            }
            pl::TransformKind::Sort { by, .. } => {
                let sorts = self.lower_sorts(by)?;
                self.pipeline.push(Transform::Sort(sorts));

                None
            }
            pl::TransformKind::Take { range, .. } => {
                let window = self.window.take().unwrap_or_default();
                let range = self.lower_range(range)?;

                validate_take_range(&range, span)?;

                self.pipeline.push(Transform::Take(rq::Take {
                    range,
                    partition: window.partition,
                    sort: window.sort,
                }));

                None
            }
            pl::TransformKind::Join {
                side, with, filter, ..
            } => {
                let with_id = with.id.unwrap();
                let with = self.lower_table_ref(*with)?;
                self.local_that_id = Some(with_id);

                let transform = Transform::Join {
                    side,
                    with,
                    filter: self.lower_expr(*filter)?,
                };
                self.pipeline.push(transform);

                Some(vec![input_id, with_id])
            }
            pl::TransformKind::Append(bottom) => {
                let bottom = self.lower_table_ref(*bottom)?;

                self.pipeline.push(Transform::Append(bottom));

                todo!()
            }
            pl::TransformKind::Loop(pipeline) => {
                let relation = self.lower_relation(*pipeline)?;
                let mut pipeline = relation.kind.into_pipeline().unwrap();

                // last select is not needed here
                pipeline.pop();

                self.pipeline.push(Transform::Loop(pipeline));

                todo!()
            }
            pl::TransformKind::Group { .. } | pl::TransformKind::Window { .. } => unreachable!(
                "transform `{}` cannot be lowered.",
                (*transform_call.kind).as_ref()
            ),
        };
        self.window = None;

        if let Some(new_fields) = new_fields {
            Ok(new_fields)
        } else {
            let input_target = self.node_mapping.get(&input_id).unwrap();
            Ok(input_target.as_relation().unwrap().clone())
        }

        // resulting transforms are stored in self.pipeline
    }

    fn lower_range(&mut self, range: Range<Box<pl::Expr>>) -> Result<Range<rq::Expr>> {
        Ok(Range {
            start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
            end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
        })
    }

    fn lower_sorts(&mut self, by: Vec<ColumnSort<Box<pl::Expr>>>) -> Result<Vec<ColumnSort<CId>>> {
        by.into_iter()
            .map(|ColumnSort { column, direction }| {
                let id = column.id.unwrap();
                self.ensure_lowered(*column, false)?;
                let column = *self.node_mapping.get(&id).unwrap().as_column().unwrap();
                Ok(ColumnSort { direction, column })
            })
            .try_collect()
    }

    /// Lowers an expression node.
    /// If expr is a tuple, this tuple will flattened into a column of a relations, arbitrarily deep.
    /// For example:
    /// expr={a = 1, b = {c = 2, d = {e = 3}}, f = 4}
    /// ... will be converted into:
    /// ids=[a, b, f], cids=[a, b.c, b.d.e, f]
    fn lower_and_flatten_tuple(
        &mut self,
        exprs: pl::Expr,
        is_aggregation: bool,
    ) -> Result<Vec<usize>> {
        if exprs.ty.as_ref().unwrap().kind.is_tuple() {
            let id = exprs.id.unwrap();
            self.ensure_lowered(exprs, is_aggregation)?;

            let ids = self.node_mapping.get(&id).unwrap().as_relation().unwrap();
            Ok(ids.clone())
        } else {
            todo!()
        }
    }

    fn flatten_tuple_fields_into_cids(&self, ids: &[usize]) -> Vec<CId> {
        let mut cids = Vec::new();
        let mut ids_rev = ids.to_vec();
        ids_rev.reverse();

        while let Some(id) = ids_rev.pop() {
            let target = self.node_mapping.get(&id).unwrap();

            match target {
                LoweredTarget::Column(cid) => cids.push(*cid),
                LoweredTarget::Relation(column_ids) => {
                    ids_rev.extend(column_ids.iter().rev());
                }
            }
        }

        cids
    }

    fn ensure_lowered(&mut self, expr_ast: pl::Expr, is_aggregation: bool) -> Result<()> {
        let id = expr_ast.id.unwrap();
        // short-circuit if this node has already been lowered
        if self.node_mapping.contains_key(&id) {
            return Ok(());
        }

        let target = match expr_ast.kind {
            pl::ExprKind::Ident(ident) => self.lookup_ident(ident).with_span(expr_ast.span)?,
            pl::ExprKind::Indirection { base, field } => {
                let base_id = base.id.unwrap();
                self.ensure_lowered(*base, is_aggregation)?;

                self.lookup_indirection(base_id, &field)
                    .with_span(expr_ast.span)?
                    .clone()
            }
            pl::ExprKind::Tuple(fields) => {
                // tuple unpacking
                let mut ids = Vec::new();
                for field in fields {
                    ids.push(field.id.unwrap());
                    self.ensure_lowered(field, is_aggregation)?;
                }
                LoweredTarget::Relation(ids)
            }
            pl::ExprKind::All { within, except } => {
                // this should never fail since it succeeded during resolution
                let field_mask = super::resolver::ty_tuple_exclusion(
                    within.ty.as_ref().unwrap(),
                    except.ty.as_ref().unwrap(),
                    None,
                    None,
                )
                .unwrap();

                // lower within
                let within_id = within.id.unwrap();
                self.ensure_lowered(*within, is_aggregation)?;
                let within_target = self.node_mapping.get(&within_id).unwrap();
                let within_ids = within_target.as_relation().ok_or_else(|| {
                    Error::new_assert("indirection on non-relation")
                        .push_hint(format!("within={within_target:?}"))
                })?;

                // apply mask
                let ids = itertools::zip_eq(within_ids, field_mask)
                    .filter(|(_, p)| *p)
                    .map(|(x, _)| *x)
                    .collect_vec();
                LoweredTarget::Relation(ids)
            }
            _ => {
                // lower expr and define a Compute
                let expr = self.lower_expr(expr_ast)?;

                // construct ColumnDef
                let cid = self.cid.gen();
                let compute = rq::Compute {
                    id: cid,
                    expr,
                    window: None,
                    is_aggregation,
                };
                self.pipeline.push(Transform::Compute(compute));

                LoweredTarget::Column(cid)
            }
        };
        self.node_mapping.insert(id, target);
        Ok(())
    }

    fn lower_expr(&mut self, expr: pl::Expr) -> Result<rq::Expr> {
        let span = expr.span;

        let kind = match expr.kind {
            pl::ExprKind::Ident(_) | pl::ExprKind::All { .. } => {
                return Err(Error::new_assert(
                    "unreachable code: should have been lowered earlier",
                )
                .with_span(span));
            }

            pl::ExprKind::Indirection { base, field } => {
                let base_id = base.id.unwrap();
                self.ensure_lowered(*base, false)?;

                let target = self
                    .lookup_indirection(base_id, &field)
                    .with_span(expr.span)?
                    .clone();

                let cid = target
                    .into_column()
                    .expect("lower_expr to refer to columns only");
                rq::ExprKind::ColumnRef(cid)
            }

            pl::ExprKind::Literal(literal) => rq::ExprKind::Literal(literal),

            pl::ExprKind::SString(items) => {
                rq::ExprKind::SString(self.lower_interpolations(items)?)
            }
            pl::ExprKind::FString(items) => {
                let mut res = None;
                for item in items {
                    let item = Some(match item {
                        pl::InterpolateItem::String(string) => str_lit(string),
                        pl::InterpolateItem::Expr { expr, .. } => self.lower_expr(*expr)?,
                    });

                    res = rq::maybe_binop(res, "std.concat", item);
                }

                res.unwrap_or_else(|| str_lit("".to_string())).kind
            }
            pl::ExprKind::Case(cases) => rq::ExprKind::Case(
                cases
                    .into_iter()
                    .map(|case| -> Result<_> {
                        Ok(SwitchCase {
                            condition: self.lower_expr(*case.condition)?,
                            value: self.lower_expr(*case.value)?,
                        })
                    })
                    .try_collect()?,
            ),
            pl::ExprKind::RqOperator { name, args } => {
                let args = args.into_iter().map(|x| self.lower_expr(x)).try_collect()?;

                rq::ExprKind::Operator { name, args }
            }
            pl::ExprKind::Param(id) => rq::ExprKind::Param(id),

            pl::ExprKind::Tuple(_) => {
                return Err(
                    Error::new_simple("table instance cannot be referenced directly")
                        .push_hint("did you forget to specify the column name?")
                        .with_span(span),
                );
            }

            pl::ExprKind::Array(exprs) => rq::ExprKind::Array(
                exprs
                    .into_iter()
                    .map(|x| self.lower_expr(x))
                    .try_collect()?,
            ),

            pl::ExprKind::FuncCall(_) | pl::ExprKind::Func(_) | pl::ExprKind::TransformCall(_) => {
                log::debug!("cannot lower {expr:?}");
                return Err(Error::new(Reason::Unexpected {
                    found: format!("`{}`", write_pl(expr.clone())),
                })
                .push_hint("this is probably a 'bad type' error (we are working on that)")
                .with_span(expr.span));
            }

            pl::ExprKind::Internal(_) => {
                return Err(Error::new_assert(format!(
                    "Unresolved lowering: {}",
                    write_pl(expr)
                )))
            }
        };

        Ok(rq::Expr { kind, span })
    }

    fn lower_interpolations(
        &mut self,
        items: Vec<InterpolateItem<pl::Expr>>,
    ) -> Result<Vec<InterpolateItem<rq::Expr>>> {
        items
            .into_iter()
            .map(|i| {
                Ok(match i {
                    InterpolateItem::String(s) => InterpolateItem::String(s),
                    InterpolateItem::Expr { expr, .. } => InterpolateItem::Expr {
                        expr: Box::new(self.lower_expr(*expr)?),
                        format: None,
                    },
                })
            })
            .try_collect()
    }

    fn lookup_ident(&self, ident: Ident) -> Result<LoweredTarget, Error> {
        if ident.path != [NS_LOCAL] {
            return Err(Error::new_assert("non-local unresolved reference"));
        }
        let target_id = match ident.name.as_str() {
            NS_THIS => self.local_this_id.as_ref(),
            NS_THAT => self.local_that_id.as_ref(),
            _ => {
                return Err(Error::new_assert(format!(
                    "unhandled local reference: {}",
                    ident.name
                )));
            }
        };
        let Some(target_id) = target_id else {
            return Err(Error::new_assert("local reference from non-local context")
                .push_hint(format!("ident={ident}")));
        };
        let Some(target) = self.node_mapping.get(target_id) else {
            return Err(
                Error::new_assert("node not lowered yet").push_hint(format!("ident={ident}"))
            );
        };

        Ok(target.clone())
    }

    fn lookup_indirection(
        &self,
        base_id: usize,
        field: &pl::IndirectionKind,
    ) -> Result<&LoweredTarget> {
        let base_target = self.node_mapping.get(&base_id).unwrap();

        let mapped_relation = base_target.as_relation().ok_or_else(|| {
            Error::new_assert("indirection on non-relation")
                .push_hint(format!("base={base_target:?}"))
                .push_hint(format!("field={field:?}"))
        })?;

        let pos = field
            .as_position()
            .expect("indirections to be resolved into positional");

        let target_id = mapped_relation
            .get(*pos as usize)
            .expect("id to be lowered");

        let target = self.node_mapping.get(target_id).ok_or_else(|| {
            Error::new_assert("node not lowered yet")
                .push_hint(format!("base_target={base_target:?}"))
                .push_hint(format!("field={field:?}"))
        })?;

        Ok(target)
    }
}

fn str_lit(string: String) -> rq::Expr {
    rq::Expr {
        kind: rq::ExprKind::Literal(pl::Literal::String(string)),
        span: None,
    }
}

fn validate_take_range(range: &Range<rq::Expr>, span: Option<Span>) -> Result<()> {
    fn bound_as_int(bound: &Option<rq::Expr>) -> Option<Option<&i64>> {
        bound
            .as_ref()
            .map(|e| e.kind.as_literal().and_then(|l| l.as_integer()))
    }

    fn bound_display(bound: Option<Option<&i64>>) -> String {
        bound
            .map(|x| x.map(|l| l.to_string()).unwrap_or_else(|| "?".to_string()))
            .unwrap_or_default()
    }

    let start = bound_as_int(&range.start);
    let end = bound_as_int(&range.end);

    let start_ok = if let Some(start) = start {
        start.map(|s| *s >= 1).unwrap_or(false)
    } else {
        true
    };

    let end_ok = if let Some(end) = end {
        end.map(|e| *e >= 1).unwrap_or(false)
    } else {
        true
    };

    if !start_ok || !end_ok {
        let range_display = format!("{}..{}", bound_display(start), bound_display(end));
        Err(Error::new(Reason::Expected {
            who: Some("take".to_string()),
            expected: "a positive int range".to_string(),
            found: range_display,
        })
        .with_span(span))
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct TableExtractor {
    path: Vec<String>,

    tables: Vec<(Ident, (decl::TableDecl, Option<usize>))>,
}

impl TableExtractor {
    /// Finds table declarations in a module, recursively.
    fn extract(root_module: &Module) -> Vec<(Ident, (decl::TableDecl, Option<usize>))> {
        let mut te = TableExtractor::default();
        te.extract_from_module(root_module);
        te.tables
    }

    /// Finds table declarations in a module, recursively.
    fn extract_from_module(&mut self, namespace: &Module) {
        for (name, entry) in &namespace.names {
            self.path.push(name.clone());

            match &entry.kind {
                DeclKind::Module(ns) => {
                    self.extract_from_module(ns);
                }
                DeclKind::TableDecl(table) => {
                    let fq_ident = Ident::from_path(self.path.clone());
                    self.tables
                        .push((fq_ident, (table.clone(), entry.declared_at)));
                }
                _ => {}
            }
            self.path.pop();
        }
    }
}

/// Does a topological sort of the pipeline definitions and prunes all definitions that
/// are not needed for the main pipeline. To do this, it needs to collect references
/// between pipelines.
fn toposort_tables(
    tables: Vec<(Ident, (decl::TableDecl, Option<usize>))>,
    main_table: &Ident,
) -> Vec<(Ident, (decl::TableDecl, Option<usize>))> {
    let tables: HashMap<_, _, RandomState> = HashMap::from_iter(tables);

    let mut dependencies: Vec<(Ident, Vec<Ident>)> = Vec::new();
    for (ident, table) in &tables {
        let deps = if let TableExpr::RelationVar(e) = &table.0.expr {
            TableDepsCollector::collect(*e.clone())
        } else {
            vec![]
        };

        dependencies.push((ident.clone(), deps));
    }

    // sort just to make sure lowering is stable
    dependencies.sort_by(|a, b| a.0.cmp(&b.0));

    let sort = toposort(&dependencies, Some(main_table)).unwrap();

    let mut tables = tables;
    sort.into_iter()
        .map(|ident| tables.remove_entry(ident).unwrap())
        .collect_vec()
}

#[derive(Default)]
struct TableDepsCollector {
    deps: Vec<Ident>,
}

impl TableDepsCollector {
    fn collect(expr: pl::Expr) -> Vec<Ident> {
        let mut c = TableDepsCollector::default();
        c.fold_expr(expr).unwrap();
        c.deps
    }
}

impl PlFold for TableDepsCollector {
    fn fold_expr(&mut self, mut expr: pl::Expr) -> Result<pl::Expr> {
        expr.kind = match expr.kind {
            pl::ExprKind::Ident(ref ident) => {
                if let Some(ty) = &expr.ty {
                    if ty.is_relation() {
                        self.deps.push(ident.clone());
                    }
                }
                expr.kind
            }
            pl::ExprKind::TransformCall(tc) => {
                pl::ExprKind::TransformCall(self.fold_transform_call(tc)?)
            }
            pl::ExprKind::Func(func) => pl::ExprKind::Func(Box::new(self.fold_func(*func)?)),

            // optimization: don't recurse into anything else than TransformCalls and Func
            _ => expr.kind,
        };
        Ok(expr)
    }
}

fn get_span_of_id(l: &Lowerer, id: Option<usize>) -> Option<Span> {
    id.and_then(|id| l.root_mod.span_map.get(&id)).cloned()
}

fn with_span_if_not_exists<'a, F>(get_span: F) -> impl FnOnce(Error) -> Error + 'a
where
    F: FnOnce() -> Option<Span> + 'a,
{
    move |e| {
        if e.span.is_some() {
            return e;
        }

        e.with_span(get_span())
    }
}
