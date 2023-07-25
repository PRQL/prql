use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::iter::zip;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::error::{Error, Reason, Span, WithErrorInfo};
use crate::generic::{ColumnSort, WindowFrame};
use crate::ir::generic::{InterpolateItem, Range, SwitchCase};
use crate::ir::pl::{
    self, BinaryExpr, Ident, Lineage, LineageColumn, PlFold, QueryDef, TupleField, UnaryExpr,
};
use crate::ir::rq::{self, CId, Query, RelationColumn, RelationLiteral, TId, TableDecl, Transform};
use crate::semantic::context::TableExpr;
use crate::semantic::module::Module;
use crate::utils::{toposort, IdGenerator};
use crate::COMPILER_VERSION;

use super::context::{self, Context, DeclKind};
use super::NS_DEFAULT_DB;

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved expressions.
pub fn lower_to_ir(context: Context, main_path: &[String]) -> Result<(Query, Context)> {
    // find main
    log::debug!("lookup for main pipeline in {main_path:?}");
    let (_, main_ident) = context.find_main_rel(main_path).map_err(|hint| {
        Error::new_simple("Missing main pipeline")
            .with_code("E0001")
            .with_hints(hint)
    })?;

    // find & validate query def
    let def = context.find_query_def(&main_ident);
    let def = def.cloned().unwrap_or_default();
    validate_query_def(&def)?;

    // find all tables in the root module
    let tables = TableExtractor::extract(&context.root_mod);

    // prune and toposort
    let tables = toposort_tables(tables, &main_ident);

    // lower tables
    let mut l = Lowerer::new(context);
    let mut main_relation = None;
    for (fq_ident, table) in tables {
        let is_main = fq_ident == main_ident;

        l.lower_table_decl(table, fq_ident)?;

        if is_main {
            let main_table = l.table_buffer.pop().unwrap();
            main_relation = Some(main_table.relation);
        }
    }

    let query = Query {
        def,
        tables: l.table_buffer,
        relation: main_relation.unwrap(),
    };
    Ok((query, l.context))
}

fn extern_ref_to_relation(
    mut columns: Vec<TupleField>,
    fq_ident: &Ident,
) -> (rq::Relation, Option<String>) {
    let extern_name = if fq_ident.starts_with_part(NS_DEFAULT_DB) {
        let (_, remainder) = fq_ident.clone().pop_front();
        remainder.unwrap()
    } else {
        // tables that are not from default_db
        todo!()
    };

    // put wildcards last
    columns.sort_by_key(|a| matches!(a, TupleField::Wildcard(_)));

    let relation = rq::Relation {
        kind: rq::RelationKind::ExternRef(extern_name),
        columns: tuple_fields_to_relation_columns(columns),
    };
    (relation, None)
}

fn tuple_fields_to_relation_columns(columns: Vec<TupleField>) -> Vec<RelationColumn> {
    columns
        .into_iter()
        .map(|field| match field {
            TupleField::Single(name, _) => RelationColumn::Single(name),
            TupleField::Wildcard(_) => RelationColumn::Wildcard,
        })
        .collect_vec()
}

fn validate_query_def(query_def: &QueryDef) -> Result<()> {
    if let Some(requirement) = &query_def.version {
        if !requirement.matches(&COMPILER_VERSION) {
            return Err(Error::new_simple("This query uses a version of PRQL that is not supported by the prql-compiler. Please upgrade the compiler.").into());
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Lowerer {
    cid: IdGenerator<CId>,
    tid: IdGenerator<TId>,

    context: Context,

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
}

#[derive(Clone, EnumAsInner, Debug)]
enum LoweredTarget {
    /// Lowered node was a computed expression.
    Compute(CId),

    /// Lowered node was a pipeline input.
    /// Contains mapping from column names to CIds, along with order in frame.
    Input(HashMap<RelationColumn, (CId, usize)>),
}

impl Lowerer {
    fn new(context: Context) -> Self {
        Lowerer {
            context,

            cid: IdGenerator::new(),
            tid: IdGenerator::new(),

            node_mapping: HashMap::new(),
            table_mapping: HashMap::new(),

            window: None,
            pipeline: Vec::new(),
            table_buffer: Vec::new(),
        }
    }

    fn lower_table_decl(&mut self, table: context::TableDecl, fq_ident: Ident) -> Result<()> {
        let context::TableDecl { ty, expr } = table;

        // TODO: can this panic?
        let columns = ty.unwrap().into_relation().unwrap();

        let (relation, name) = match expr {
            TableExpr::RelationVar(expr) => {
                // a CTE
                (self.lower_relation(*expr)?, Some(fq_ident.name.clone()))
            }
            TableExpr::LocalTable => extern_ref_to_relation(columns, &fq_ident),
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
        let mut expr = expr;
        if expr.lineage.is_none() {
            // make sure that type of this expr has been inferred to be a table
            expr.lineage = Some(Lineage::default());
        }

        Ok(match expr.kind {
            pl::ExprKind::Ident(fq_table_name) => {
                // ident that refer to table: create an instance of the table
                let id = expr.id.unwrap();
                let tid = *self.table_mapping.get(&fq_table_name).unwrap();

                log::debug!("lowering an instance of table {fq_table_name} (id={id})...");

                let input_name = expr
                    .lineage
                    .as_ref()
                    .and_then(|f| f.inputs.first())
                    .map(|i| i.name.clone());
                let name = input_name.or(Some(fq_table_name.name));

                self.create_a_table_instance(id, name, tid)
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
                let table_ref = self.create_a_table_instance(id, None, tid);

                let redirects = zip(cids, table_ref.columns.iter().map(|(_, c)| *c)).collect();
                self.redirect_mappings(redirects);

                table_ref
            }
            pl::ExprKind::SString(items) => {
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                // pull columns from the table decl
                let frame = expr.lineage.as_ref().unwrap();
                let input = frame.inputs.get(0).unwrap();

                let table_decl = self.context.root_mod.get(&input.table).unwrap();
                let table_decl = table_decl.kind.as_table_decl().unwrap();
                let ty = table_decl.ty.as_ref();
                // TODO: can this panic?
                let columns = ty.unwrap().as_relation().unwrap().clone();

                log::debug!("lowering sstring table, columns = {columns:?}");

                // lower the expr
                let items = self.lower_interpolations(items)?;
                let relation = rq::Relation {
                    kind: rq::RelationKind::SString(items),
                    columns: tuple_fields_to_relation_columns(columns),
                };

                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                self.create_a_table_instance(id, None, tid)
            }
            pl::ExprKind::RqOperator { name, args } => {
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                // pull columns from the table decl
                let frame = expr.lineage.as_ref().unwrap();
                let input = frame.inputs.get(0).unwrap();

                let table_decl = self.context.root_mod.get(&input.table).unwrap();
                let table_decl = table_decl.kind.as_table_decl().unwrap();
                let ty = table_decl.ty.as_ref();
                // TODO: can this panic?
                let columns = ty.unwrap().as_relation().unwrap().clone();

                log::debug!("lowering function table, columns = {columns:?}");

                // lower the expr
                let args = args.into_iter().map(|a| self.lower_expr(a)).try_collect()?;
                let relation = rq::Relation {
                    kind: rq::RelationKind::BuiltInFunction { name, args },
                    columns: tuple_fields_to_relation_columns(columns),
                };

                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                self.create_a_table_instance(id, None, tid)
            }

            pl::ExprKind::Array(elements) => {
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                // pull columns from the table decl
                let frame = expr.lineage.as_ref().unwrap();
                let columns = (frame.columns.iter())
                    .map(|c| {
                        RelationColumn::Single(
                            c.as_single().unwrap().0.as_ref().map(|i| i.name.clone()),
                        )
                    })
                    .collect_vec();

                let lit = RelationLiteral {
                    columns: columns
                        .iter()
                        .map(|c| c.as_single().unwrap().clone().unwrap())
                        .collect_vec(),
                    rows: elements
                        .into_iter()
                        .map(|row| {
                            row.kind
                                .into_tuple()
                                .unwrap()
                                .into_iter()
                                .map(|element| element.kind.into_literal().unwrap())
                                .collect()
                        })
                        .collect(),
                };

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
            }

            _ => {
                return Err(Error::new(Reason::Expected {
                    who: None,
                    expected: "a pipeline that resolves to a table".to_string(),
                    found: format!("`{expr}`"),
                })
                .push_hint("are you missing `from` statement?")
                .with_span(expr.span)
                .into())
            }
        })
    }

    fn redirect_mappings(&mut self, redirects: HashMap<CId, CId>) {
        for target in self.node_mapping.values_mut() {
            match target {
                LoweredTarget::Compute(cid) => {
                    if let Some(new) = redirects.get(cid) {
                        *cid = *new;
                    }
                }
                LoweredTarget::Input(mapping) => {
                    for (cid, _) in mapping.values_mut() {
                        if let Some(new) = redirects.get(cid) {
                            *cid = *new;
                        }
                    }
                }
            }
        }
    }

    fn create_a_table_instance(
        &mut self,
        id: usize,
        name: Option<String>,
        tid: TId,
    ) -> rq::TableRef {
        // create instance columns from table columns
        let table = self.table_buffer.iter().find(|t| t.id == tid).unwrap();

        let columns = (table.relation.columns.iter())
            .cloned()
            .unique()
            .map(|col| (col, self.cid.gen()))
            .collect_vec();

        log::debug!("... columns = {:?}", columns);

        let input_cids: HashMap<_, _> = columns
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, (col, cid))| (col, (cid, index)))
            .collect();
        self.node_mapping
            .insert(id, LoweredTarget::Input(input_cids));
        rq::TableRef {
            source: tid,
            name,
            columns,
        }
    }

    fn lower_relation(&mut self, expr: pl::Expr) -> Result<rq::Relation> {
        let lineage = expr.lineage.clone();
        let prev_pipeline = self.pipeline.drain(..).collect_vec();

        self.lower_pipeline(expr, None)?;

        let mut transforms = self.pipeline.drain(..).collect_vec();
        let columns = self.push_select(lineage, &mut transforms)?;

        self.pipeline = prev_pipeline;

        let relation = rq::Relation {
            kind: rq::RelationKind::Pipeline(transforms),
            columns,
        };
        Ok(relation)
    }

    // Result is stored in self.pipeline
    fn lower_pipeline(&mut self, ast: pl::Expr, closure_param: Option<usize>) -> Result<()> {
        let transform_call = match ast.kind {
            pl::ExprKind::TransformCall(transform) => transform,
            pl::ExprKind::Func(closure) => {
                let param = closure.params.first();
                let param = param.and_then(|p| p.name.parse::<usize>().ok());
                return self.lower_pipeline(*closure.body, param);
            }
            _ => {
                if let Some(target) = ast.target_id {
                    if Some(target) == closure_param {
                        // ast is a closure param, so we can skip pushing From
                        return Ok(());
                    }
                }

                let table_ref = self.lower_table_ref(ast)?;
                self.pipeline.push(Transform::From(table_ref));
                return Ok(());
            }
        };

        // lower input table
        self.lower_pipeline(*transform_call.input, closure_param)?;

        // ... and continues with transforms created in this function

        let window = rq::Window {
            frame: WindowFrame {
                kind: transform_call.frame.kind,
                range: self.lower_range(transform_call.frame.range)?,
            },
            partition: self.declare_as_columns(transform_call.partition, false)?,
            sort: self.lower_sorts(transform_call.sort)?,
        };
        self.window = Some(window);

        match *transform_call.kind {
            pl::TransformKind::Derive { assigns, .. } => {
                self.declare_as_columns(assigns, false)?;
            }
            pl::TransformKind::Select { assigns, .. } => {
                let cids = self.declare_as_columns(assigns, false)?;
                self.pipeline.push(Transform::Select(cids));
            }
            pl::TransformKind::Filter { filter, .. } => {
                let filter = self.lower_expr(*filter)?;

                self.pipeline.push(Transform::Filter(filter));
            }
            pl::TransformKind::Aggregate { assigns, .. } => {
                let window = self.window.take();

                let compute = self.declare_as_columns(assigns, true)?;

                let partition = window.unwrap().partition;
                self.pipeline
                    .push(Transform::Aggregate { partition, compute });
            }
            pl::TransformKind::Sort { by, .. } => {
                let sorts = self.lower_sorts(by)?;
                self.pipeline.push(Transform::Sort(sorts));
            }
            pl::TransformKind::Take { range, .. } => {
                let window = self.window.take().unwrap_or_default();
                let range = self.lower_range(range)?;

                validate_take_range(&range, ast.span)?;

                self.pipeline.push(Transform::Take(rq::Take {
                    range,
                    partition: window.partition,
                    sort: window.sort,
                }));
            }
            pl::TransformKind::Join {
                side, with, filter, ..
            } => {
                let with = self.lower_table_ref(*with)?;

                let transform = Transform::Join {
                    side,
                    with,
                    filter: self.lower_expr(*filter)?,
                };
                self.pipeline.push(transform);
            }
            pl::TransformKind::Append(bottom) => {
                let bottom = self.lower_table_ref(*bottom)?;

                self.pipeline.push(Transform::Append(bottom));
            }
            pl::TransformKind::Loop(pipeline) => {
                let relation = self.lower_relation(*pipeline)?;
                let mut pipeline = relation.kind.into_pipeline().unwrap();

                // last select is not needed here
                pipeline.pop();

                self.pipeline.push(Transform::Loop(pipeline));
            }
            pl::TransformKind::Group { .. } | pl::TransformKind::Window { .. } => unreachable!(
                "transform `{}` cannot be lowered.",
                (*transform_call.kind).as_ref()
            ),
        }
        self.window = None;

        // result is stored in self.pipeline
        Ok(())
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
                let column = self.declare_as_column(*column, false)?;
                Ok(ColumnSort { direction, column })
            })
            .try_collect()
    }

    /// Append a Select of final table columns derived from frame
    fn push_select(
        &mut self,
        lineage: Option<Lineage>,
        transforms: &mut Vec<Transform>,
    ) -> Result<Vec<RelationColumn>> {
        let lineage = lineage.unwrap_or_default();

        log::debug!("push_select of a frame: {:?}", lineage);

        let mut columns = Vec::new();

        // normal columns
        for col in &lineage.columns {
            match col {
                LineageColumn::Single {
                    name,
                    target_id,
                    target_name,
                } => {
                    let cid = self.lookup_cid(*target_id, target_name.as_ref())?;

                    let name = name.as_ref().map(|i| i.name.clone());
                    columns.push((RelationColumn::Single(name), cid));
                }
                LineageColumn::All { input_name, except } => {
                    let input = lineage.find_input(input_name).unwrap();

                    match &self.node_mapping[&input.id] {
                        LoweredTarget::Compute(_cid) => unreachable!(),
                        LoweredTarget::Input(input_cols) => {
                            let mut input_cols = input_cols
                                .iter()
                                .filter(|(c, _)| match c {
                                    RelationColumn::Single(Some(name)) => !except.contains(name),
                                    _ => true,
                                })
                                .collect_vec();
                            input_cols.sort_by_key(|e| e.1 .1);

                            for (col, (cid, _)) in input_cols {
                                columns.push((col.clone(), *cid));
                            }
                        }
                    }
                }
            }
        }

        let (cols, cids) = columns.into_iter().unzip();

        log::debug!("... cids={:?}", cids);
        transforms.push(Transform::Select(cids));

        Ok(cols)
    }

    fn declare_as_columns(
        &mut self,
        exprs: Vec<pl::Expr>,
        is_aggregation: bool,
    ) -> Result<Vec<CId>> {
        let mut r = Vec::with_capacity(exprs.len());
        for expr in exprs {
            let pl::ExprKind::All { except, .. } = expr.kind else {
                // base case
                r.push(self.declare_as_column(expr, is_aggregation)?);
                continue;
            };

            // special case: ExprKind::All
            let mut selected = Vec::<CId>::new();
            for target_id in expr.target_ids {
                match &self.node_mapping[&target_id] {
                    LoweredTarget::Compute(cid) => {
                        selected.push(*cid);
                    }
                    LoweredTarget::Input(input) => {
                        let mut cols = input.iter().collect_vec();
                        cols.sort_by_key(|c| c.1 .1);
                        selected.extend(cols.into_iter().map(|(_, (cid, _))| cid));
                    }
                }
            }

            let except: HashSet<CId> = except
                .into_iter()
                .filter(|e| e.target_id.is_some())
                .map(|e| {
                    let id = e.target_id.unwrap();
                    self.lookup_cid(id, Some(&e.kind.into_ident().unwrap().name))
                })
                .try_collect()?;
            selected.retain(|c| !except.contains(c));

            r.extend(selected);
        }
        Ok(r)
    }

    fn declare_as_column(
        &mut self,
        mut expr_ast: pl::Expr,
        is_aggregation: bool,
    ) -> Result<rq::CId> {
        // short-circuit if this node has already been lowered
        if let Some(LoweredTarget::Compute(lowered)) = self.node_mapping.get(&expr_ast.id.unwrap())
        {
            return Ok(*lowered);
        }

        // copy metadata before lowering
        let alias = expr_ast.alias.clone();
        let has_alias = alias.is_some();
        let needs_window = expr_ast.needs_window;
        expr_ast.needs_window = false;
        let alias_for = if has_alias {
            expr_ast.kind.as_ident().map(|x| x.name.clone())
        } else {
            None
        };
        let id = expr_ast.id.unwrap();

        // lower
        let expr = self.lower_expr(expr_ast)?;

        // don't create new ColumnDef if expr is just a ColumnRef with no renaming
        if let rq::ExprKind::ColumnRef(cid) = &expr.kind {
            if !needs_window && (!has_alias || alias == alias_for) {
                self.node_mapping.insert(id, LoweredTarget::Compute(*cid));
                return Ok(*cid);
            }
        }

        // determine window
        let window = if needs_window {
            self.window.clone()
        } else {
            None
        };

        // construct ColumnDef
        let cid = self.cid.gen();
        let compute = rq::Compute {
            id: cid,
            expr,
            window,
            is_aggregation,
        };
        self.node_mapping.insert(id, LoweredTarget::Compute(cid));

        self.pipeline.push(Transform::Compute(compute));
        Ok(cid)
    }

    fn lower_expr(&mut self, ast: pl::Expr) -> Result<rq::Expr> {
        if ast.needs_window {
            let span = ast.span;
            let cid = self.declare_as_column(ast, false)?;

            let kind = rq::ExprKind::ColumnRef(cid);
            return Ok(rq::Expr { kind, span });
        }

        let kind = match ast.kind {
            pl::ExprKind::Ident(ident) => {
                log::debug!("lowering ident {ident} (target {:?})", ast.target_id);

                if let Some(id) = ast.target_id {
                    let cid = self.lookup_cid(id, Some(&ident.name))?;

                    rq::ExprKind::ColumnRef(cid)
                } else {
                    // This is an unresolved ident.
                    // Let's hope that the database engine can resolve it.
                    rq::ExprKind::SString(vec![InterpolateItem::String(ident.name)])
                }
            }
            pl::ExprKind::All { except, .. } => {
                let mut targets = Vec::new();

                for target_id in &ast.target_ids {
                    match self.node_mapping.get(target_id) {
                        Some(LoweredTarget::Compute(cid)) => targets.push(*cid),
                        Some(LoweredTarget::Input(input_columns)) => {
                            targets.extend(input_columns.values().map(|(c, _)| c))
                        }
                        _ => {}
                    }
                }

                // this is terrible code
                let except: HashSet<_> = except
                    .iter()
                    .map(|e| {
                        let ident = e.kind.as_ident().unwrap();
                        self.lookup_cid(e.target_id.unwrap(), Some(&ident.name))
                            .unwrap()
                    })
                    .collect();

                targets.retain(|t| !except.contains(t));

                if targets.len() == 1 {
                    rq::ExprKind::ColumnRef(targets[0])
                } else {
                    return Err(
                        Error::new_simple("This wildcard usage is not yet supported.")
                            .with_span(ast.span)
                            .into(),
                    );
                }
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

            pl::ExprKind::FuncCall(_)
            | pl::ExprKind::Range(_)
            | pl::ExprKind::Tuple(_)
            | pl::ExprKind::Array(_)
            | pl::ExprKind::Func(_)
            | pl::ExprKind::Pipeline(_)
            | pl::ExprKind::Type(_)
            | pl::ExprKind::TransformCall(_) => {
                log::debug!("cannot lower {ast:?}");
                return Err(Error::new(Reason::Unexpected {
                    found: format!("`{ast}`"),
                })
                .push_hint("this is probably a 'bad type' error (we are working on that)")
                .with_span(ast.span)
                .into());
            }

            pl::ExprKind::Unary(UnaryExpr { .. })
            | pl::ExprKind::Binary(BinaryExpr { .. })
            | pl::ExprKind::Internal(_) => {
                panic!("Unresolved lowering: {ast}")
            }
        };

        Ok(rq::Expr {
            kind,
            span: ast.span,
        })
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

    fn lookup_cid(&mut self, id: usize, name: Option<&String>) -> Result<CId> {
        let cid = match self.node_mapping.get(&id) {
            Some(LoweredTarget::Compute(cid)) => *cid,
            Some(LoweredTarget::Input(input_columns)) => {
                let name = match name {
                    Some(v) => RelationColumn::Single(Some(v.clone())),
                    None => return Err(Error::new_simple(
                        "This table contains unnamed columns that need to be referenced by name",
                    )
                    .with_span(self.context.span_map.get(&id).cloned())
                    .push_hint("the name may have been overridden later in the pipeline.")
                    .into()),
                };
                log::trace!("lookup cid of name={name:?} in input {input_columns:?}");

                if let Some((cid, _)) = input_columns.get(&name) {
                    *cid
                } else {
                    panic!("cannot find cid by id={id} and name={name:?}");
                }
            }
            None => panic!("cannot find cid by id={id}"),
        };

        Ok(cid)
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
            .unwrap_or_else(|| "".to_string())
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
        .with_span(span)
        .into())
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct TableExtractor {
    path: Vec<String>,

    tables: Vec<(Ident, context::TableDecl)>,
}

impl TableExtractor {
    /// Finds table declarations in a module, recursively.
    fn extract(root_module: &Module) -> Vec<(Ident, context::TableDecl)> {
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
                    self.tables.push((fq_ident, table.clone()));
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
    tables: Vec<(Ident, context::TableDecl)>,
    main_table: &Ident,
) -> Vec<(Ident, context::TableDecl)> {
    let tables: HashMap<_, _, RandomState> = HashMap::from_iter(tables);

    let mut dependencies: Vec<(Ident, Vec<Ident>)> = Vec::new();
    for (ident, table) in &tables {
        let deps = if let TableExpr::RelationVar(e) = &table.expr {
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
