use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::pl::fold::AstFold;
use crate::ast::pl::{
    self, Expr, ExprKind, FrameColumn, Ident, InterpolateItem, Range, TableExternRef, Ty,
    WindowFrame,
};
use crate::ast::rq::{self, CId, ColumnDecl, ColumnDeclKind, Query, TId, TableDecl, Transform};
use crate::error::{Error, Reason};
use crate::semantic::module::Module;
use crate::utils::{toposort, IdGenerator};

use super::context::{self, Context, DeclKind, TableColumn, TableFrame};

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved
pub fn lower_ast_to_ir(statements: Vec<pl::Stmt>, context: Context) -> Result<Query> {
    let mut l = Lowerer::new(context);

    let mut tables = TableExtractor::extract(&mut l)?;

    let mut query_def = None;
    let mut main_pipeline = None;

    for statement in statements {
        match statement.kind {
            pl::StmtKind::QueryDef(def) => query_def = Some(def),
            pl::StmtKind::Pipeline(expr) => {
                let (ir, _) = l.lower_relation(*expr)?;
                main_pipeline = Some(ir);
            }
            pl::StmtKind::FuncDef(_) | pl::StmtKind::TableDef(_) => {}
        }
    }

    tables.extend(l.table_buffer);

    Ok(Query {
        def: query_def.unwrap_or_default(),
        tables,
        relation: main_pipeline
            .ok_or_else(|| Error::new(Reason::Simple("missing main pipeline".to_string())))?,
    })
}

struct Lowerer {
    cid: IdGenerator<CId>,
    tid: IdGenerator<TId>,

    context: Context,

    // current window for any new column defs
    window: Option<rq::Window>,

    /// describes what has certain id has been lowered to
    column_mapping: HashMap<usize, ColumnTarget>,

    // used when a pipeline is rewritten (split) during lowering
    column_redirects: HashMap<CId, CId>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_mapping: HashMap<Ident, TId>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_columns: HashMap<TId, TableColumns>,

    /// A buffer to be added into current pipeline
    pipeline: Vec<Transform>,

    /// A buffer to be added into query tables
    table_buffer: Vec<TableDecl>,
}

#[derive(Clone)]
enum ColumnTarget {
    Compute(CId),
    Input(HashMap<String, CId>),
}

type TableColumns = Vec<(String, CId)>;

impl Lowerer {
    fn new(context: Context) -> Self {
        Lowerer {
            context,
            window: None,

            cid: IdGenerator::new(),
            tid: IdGenerator::new(),

            column_mapping: HashMap::new(),
            column_redirects: HashMap::new(),
            table_mapping: HashMap::new(),
            table_columns: HashMap::new(),

            pipeline: Vec::new(),
            table_buffer: Vec::new(),
        }
    }

    /// Lower an expression into a instance of a table in the query
    fn lower_table_ref(&mut self, expr: Expr) -> Result<rq::TableRef> {
        Ok(match expr.kind {
            ExprKind::Ident(fq_table_name) => {
                // ident that refer to table: create an instance of the table
                let id = expr.id.unwrap();
                let tid = self.ensure_table_id(&fq_table_name);

                log::debug!("lowering an instance of table {fq_table_name} (id={id})...");

                let name = expr.alias.clone().or(Some(fq_table_name.name));

                let (table_ref, _) = self.create_a_table_instance(id, name, tid);
                table_ref
            }
            ExprKind::TransformCall(_) => {
                // pipeline that has to be pulled out into a table
                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                let (relation, cols) = self.lower_relation(expr)?;

                log::debug!("lowering inline table, columns = {:?}", cols);
                self.table_columns.insert(tid, cols);
                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                let (table_ref, redirects) = self.create_a_table_instance(id, None, tid);
                self.column_redirects.extend(redirects);
                table_ref
            }
            ExprKind::SString(items) => {
                if items.iter().any(|i| matches!(i, InterpolateItem::Expr(_))) {
                    bail!(Error::new(Reason::Simple(
                        "table s-strings cannot contain interpolations".to_string(),
                    ))
                    .with_help("are you missing `from` statement?")
                    .with_span(expr.span))
                }

                let id = expr.id.unwrap();

                // create a new table
                let tid = self.tid.gen();

                let wildcard = ColumnDecl {
                    id: self.cid.gen(),
                    kind: ColumnDeclKind::Wildcard,
                    window: None,
                    is_aggregation: false,
                };
                let cols: TableColumns = vec![("*".to_string(), wildcard.id)];

                let items = self.lower_interpolations(items)?;
                let relation = rq::Relation::SString(items, vec![wildcard]);

                log::debug!("lowering sstring table, columns = {:?}", cols);
                self.table_columns.insert(tid, cols);
                self.table_buffer.push(TableDecl {
                    id: tid,
                    name: None,
                    relation,
                });

                // return an instance of this new table
                let (table_ref, redirects) = self.create_a_table_instance(id, None, tid);
                self.column_redirects.extend(redirects);
                table_ref
            }
            _ => {
                bail!(Error::new(Reason::Expected {
                    who: None,
                    expected: "pipeline that resolves to a table".to_string(),
                    found: format!("`{expr}`")
                })
                .with_help("are you missing `from` statement?")
                .with_span(expr.span))
            }
        })
    }

    fn create_a_table_instance(
        &mut self,
        id: usize,
        name: Option<String>,
        tid: TId,
    ) -> (rq::TableRef, HashMap<CId, CId>) {
        // create instance columns from table columns
        let mut columns = Vec::new();
        let mut cids_by_name = HashMap::new();
        let mut cid_mapping = HashMap::new();
        for (name, table_cid) in &self.table_columns[&tid] {
            let local_cid = self.cid.gen();
            cid_mapping.insert(*table_cid, local_cid);
            cids_by_name.insert(name.clone(), local_cid);

            let kind = if name == "*" {
                ColumnDeclKind::Wildcard
            } else {
                ColumnDeclKind::Expr {
                    name: Some(name.clone()),
                    expr: rq::Expr {
                        kind: rq::ExprKind::ColumnRef(*table_cid),
                        span: None,
                    },
                }
            };
            columns.push(ColumnDecl {
                id: local_cid,
                kind,
                window: None,
                is_aggregation: false,
            });
        }
        log::debug!("... columns = {:?}", cids_by_name);
        self.column_mapping
            .insert(id, ColumnTarget::Input(cids_by_name));
        let table_ref = rq::TableRef {
            source: tid,
            name,
            columns,
        };
        (table_ref, cid_mapping)
    }

    fn ensure_table_id(&mut self, fq_ident: &Ident) -> TId {
        *self
            .table_mapping
            .entry(fq_ident.clone())
            .or_insert_with(|| self.tid.gen())
    }

    fn lower_relation(&mut self, expr: Expr) -> Result<(rq::Relation, TableColumns)> {
        let ty = expr.ty.clone();
        let prev_pipeline = self.pipeline.drain(..).collect_vec();

        self.lower_pipeline(expr)?;

        let mut transforms = self.pipeline.drain(..).collect_vec();
        let cols = self.push_select(ty, &mut transforms)?;

        self.pipeline = prev_pipeline;
        Ok((rq::Relation::Pipeline(transforms), cols))
    }

    // Result is stored in self.pipeline
    fn lower_pipeline(&mut self, ast: pl::Expr) -> Result<()> {
        let transform_call = match ast.kind {
            pl::ExprKind::TransformCall(transform) => transform,
            _ => {
                let table_ref = self.lower_table_ref(ast)?;
                self.pipeline.push(Transform::From(table_ref));
                return Ok(());
            }
        };

        // lower input table
        self.lower_pipeline(*transform_call.input)?;

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
                let select = self.declare_as_columns(assigns, false)?;
                self.pipeline.push(Transform::Select(select));
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
                let range = Range {
                    start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
                    end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
                };

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
            pl::TransformKind::Group { .. } | pl::TransformKind::Window { .. } => unreachable!(
                "transform `{}` cannot be lowered.",
                (*transform_call.kind).as_ref()
            ),
        }
        self.window = None;

        // result is stored in self.pipeline
        Ok(())
    }

    fn lower_range(&mut self, range: pl::Range<Box<pl::Expr>>) -> Result<Range<rq::Expr>> {
        Ok(Range {
            start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
            end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
        })
    }

    fn lower_sorts(&mut self, by: Vec<pl::ColumnSort>) -> Result<Vec<pl::ColumnSort<CId>>> {
        by.into_iter()
            .map(|pl::ColumnSort { column, direction }| {
                let column = self.declare_as_column(column, false)?;
                Ok(pl::ColumnSort { direction, column })
            })
            .try_collect()
    }

    /// Append a Select of final table columns derived from frame
    fn push_select(
        &mut self,
        ty: Option<pl::Ty>,
        transforms: &mut Vec<Transform>,
    ) -> Result<TableColumns> {
        let frame = ty.unwrap().into_table().unwrap_or_default();

        log::debug!("push_select of a frame: {:?}", frame);

        let mut columns = Vec::new();
        let mut in_wildcards = HashSet::new();

        // wildcards
        for col in &frame.columns {
            if let FrameColumn::Wildcard { input_name } = col {
                let input = frame.find_input(input_name).unwrap();

                match &self.column_mapping[&input.id] {
                    ColumnTarget::Compute(_cid) => unreachable!(),
                    ColumnTarget::Input(input_cols) => {
                        for (name, cid) in input_cols {
                            let cid = self.column_redirects.get(cid).unwrap_or(cid);

                            in_wildcards.insert(*cid);
                            columns.push((Some(name.clone()), *cid));
                        }
                    }
                }
            }
        }

        // normal columns
        for col in &frame.columns {
            if let FrameColumn::Single { name, expr_id } = col {
                let name = name.clone().map(|n| n.name);
                let cid = self.lookup_cid(*expr_id, name.as_ref())?;

                columns.push((name, cid));
            }
        }

        // deduplicate
        let mut cids = Vec::new();
        let mut names = Vec::new();
        for (name, cid) in columns {
            if !cids.contains(&cid) {
                if name.as_deref().unwrap_or_default() == "*" || !in_wildcards.contains(&cid) {
                    cids.push(cid);
                }
                if let Some(name) = name {
                    names.push((name, cid));
                }
            }
        }

        log::debug!("... cids={:?}", cids);
        transforms.push(Transform::Select(cids));

        Ok(names)
    }

    fn declare_as_columns(
        &mut self,
        exprs: Vec<pl::Expr>,
        is_aggregation: bool,
    ) -> Result<Vec<CId>> {
        exprs
            .into_iter()
            .map(|x| self.declare_as_column(x, is_aggregation))
            .try_collect()
    }

    fn declare_as_column(
        &mut self,
        mut expr_ast: pl::Expr,
        is_aggregation: bool,
    ) -> Result<rq::CId> {
        // copy metadata before lowering
        let alias = expr_ast.alias.clone();
        let has_alias = alias.is_some();
        let needs_window = expr_ast.needs_window;
        expr_ast.needs_window = false;
        let name = if let Some(alias) = expr_ast.alias.clone() {
            Some(alias)
        } else {
            expr_ast.kind.as_ident().map(|x| x.name.clone())
        };
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
                self.column_mapping.insert(id, ColumnTarget::Compute(*cid));
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
        let decl = ColumnDecl {
            id: cid,
            kind: ColumnDeclKind::Expr { name, expr },
            window,
            is_aggregation,
        };
        self.column_mapping.insert(id, ColumnTarget::Compute(cid));

        self.pipeline.push(Transform::Compute(decl));
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
            pl::ExprKind::Literal(literal) => rq::ExprKind::Literal(literal),
            pl::ExprKind::Range(Range { start, end }) => rq::ExprKind::Range(Range {
                start: start
                    .map(|x| self.lower_expr(*x))
                    .transpose()?
                    .map(Box::new),
                end: end.map(|x| self.lower_expr(*x)).transpose()?.map(Box::new),
            }),
            pl::ExprKind::Binary { left, op, right } => rq::ExprKind::Binary {
                left: Box::new(self.lower_expr(*left)?),
                op,
                right: Box::new(self.lower_expr(*right)?),
            },
            pl::ExprKind::Unary { op, expr } => rq::ExprKind::Unary {
                op: match op {
                    pl::UnOp::Neg => rq::UnOp::Neg,
                    pl::UnOp::Not => rq::UnOp::Not,
                    pl::UnOp::EqSelf => bail!("Cannot lower to IR expr: `{op:?}`"),
                },
                expr: Box::new(self.lower_expr(*expr)?),
            },
            pl::ExprKind::SString(items) => {
                rq::ExprKind::SString(self.lower_interpolations(items)?)
            }
            pl::ExprKind::FString(items) => {
                rq::ExprKind::FString(self.lower_interpolations(items)?)
            }
            pl::ExprKind::FuncCall(_)
            | pl::ExprKind::Closure(_)
            | pl::ExprKind::List(_)
            | pl::ExprKind::Pipeline(_)
            | pl::ExprKind::TransformCall(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
        };

        Ok(rq::Expr {
            kind,
            span: ast.span,
        })
    }

    fn lower_interpolations(
        &mut self,
        items: Vec<InterpolateItem>,
    ) -> Result<Vec<InterpolateItem<rq::Expr>>> {
        items
            .into_iter()
            .map(|i| {
                Ok(match i {
                    InterpolateItem::String(s) => InterpolateItem::String(s),
                    InterpolateItem::Expr(e) => {
                        InterpolateItem::Expr(Box::new(self.lower_expr(*e)?))
                    }
                })
            })
            .try_collect()
    }

    fn lookup_cid(&self, id: usize, name: Option<&String>) -> Result<CId> {
        let cid = match self.column_mapping.get(&id).cloned() {
            Some(ColumnTarget::Compute(cid)) => cid,
            Some(ColumnTarget::Input(input_columns)) => {
                let name = match name {
                    Some(v) => v,
                    None => bail!(Error::new(Reason::Simple(
                        "This table contains unnamed columns, that need to be referenced by name"
                            .to_string()
                    ))
                    .with_span(self.context.span_map.get(&id).cloned())),
                };
                log::trace!("lookup cid of name={name:?} in input {input_columns:?}");

                if let Some(cid) = input_columns.get(name).or_else(|| input_columns.get("*")) {
                    *cid
                } else {
                    panic!("cannot find cid by id={id} and name={name:?}");
                }
            }
            None => panic!("cannot find cid by id={id}"),
        };

        Ok(self.column_redirects.get(&cid).cloned().unwrap_or(cid))
    }
}

// Collects all ExternRefs and
#[derive(Default)]
struct TableExtractor {
    path: Vec<String>,

    tables: Vec<(Ident, context::TableDecl)>,
}

impl TableExtractor {
    fn extract(lowerer: &mut Lowerer) -> Result<Vec<TableDecl>> {
        let mut te = TableExtractor::default();

        te.extract_from_namespace(&lowerer.context.root_mod);

        let tables = toposort_tables(te.tables);

        (tables.into_iter())
            .map(|(fq_ident, table)| lower_table(lowerer, table, fq_ident))
            .try_collect()
    }

    fn extract_from_namespace(&mut self, namespace: &Module) {
        for (name, entry) in &namespace.names {
            self.path.push(name.clone());

            match &entry.kind {
                DeclKind::Module(ns) => {
                    self.extract_from_namespace(ns);
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

fn lower_table(
    lowerer: &mut Lowerer,
    table: context::TableDecl,
    fq_ident: Ident,
) -> Result<TableDecl> {
    let id = lowerer.ensure_table_id(&fq_ident);

    let context::TableDecl { frame, expr } = table;

    let (expr, cols) = if let Some(expr) = expr {
        // this is a CTE
        lowerer.lower_relation(*expr)?
    } else {
        lower_extern_table(lowerer, frame, &fq_ident)
    };
    let name = Some(fq_ident.name.clone());

    log::debug!("lowering table {name:?}, columns = {:?}", cols);
    lowerer.table_columns.insert(id, cols);

    Ok(TableDecl {
        id,
        name,
        relation: expr,
    })
}

fn lower_extern_table(
    lowerer: &mut Lowerer,
    frame: TableFrame,
    fq_ident: &Ident,
) -> (rq::Relation, TableColumns) {
    let column_defs = (frame.columns.iter())
        .map(|col| ColumnDecl {
            id: lowerer.cid.gen(),
            kind: match col {
                TableColumn::Wildcard => ColumnDeclKind::Wildcard,
                TableColumn::Single(name) => ColumnDeclKind::ExternRef(name.clone().unwrap()),
            },
            window: None,
            is_aggregation: false,
        })
        .collect_vec();

    let cols = column_defs
        .iter()
        .map(|cd| match &cd.kind {
            ColumnDeclKind::Wildcard => ("*".to_string(), cd.id),
            ColumnDeclKind::ExternRef(name) => (name.clone(), cd.id),
            ColumnDeclKind::Expr { .. } => unreachable!(),
        })
        .collect();
    let expr = rq::Relation::ExternRef(
        TableExternRef::LocalTable(fq_ident.name.clone()),
        column_defs,
    );
    (expr, cols)
}

fn toposort_tables(tables: Vec<(Ident, context::TableDecl)>) -> Vec<(Ident, context::TableDecl)> {
    let tables: HashMap<_, _, RandomState> = HashMap::from_iter(tables);

    let mut dependencies: Vec<(Ident, Vec<Ident>)> = tables
        .iter()
        .map(|(ident, table)| {
            let deps = (table.expr.clone())
                .map(|e| TableDepsCollector::collect(*e))
                .unwrap_or_default();
            (ident.clone(), deps)
        })
        .collect();
    dependencies.sort_by(|a, b| a.0.cmp(&b.0));

    let sort = toposort(&dependencies).unwrap();

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

impl AstFold for TableDepsCollector {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        expr.kind = match expr.kind {
            pl::ExprKind::Ident(ref ident) => {
                if let Some(Ty::Table(_)) = &expr.ty {
                    self.deps.push(ident.clone());
                }
                expr.kind
            }
            pl::ExprKind::TransformCall(tc) => {
                pl::ExprKind::TransformCall(self.fold_transform_call(tc)?)
            }
            _ => expr.kind,
        };
        Ok(expr)
    }
}
