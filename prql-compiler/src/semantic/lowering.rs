use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::pl::{
    self, Expr, FrameColumn, Ident, InterpolateItem, Range, TableExternRef, WindowFrame,
};
use crate::ast::rq::{self, CId, ColumnDecl, ColumnDefKind, Query, TId, TableDecl, Transform};
use crate::error::{Error, Reason};
use crate::semantic::module::Module;
use crate::utils::IdGenerator;

use super::context::{Context, DeclKind, TableColumn, TableFrame};

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved
pub fn lower_ast_to_ir(statements: Vec<pl::Stmt>, context: Context) -> Result<Query> {
    let mut l = Lowerer::new(context);

    // TODO: when extern refs will be resolved to a local instance of a table
    // instead of a the global table definition, this could be removed
    let tables = TableExtractor::extract(&mut l)?;

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

    /// mapping from [Expr].id into [CId]s
    column_mapping: HashMap<usize, CId>,

    input_mapping: HashMap<usize, HashMap<String, CId>>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_mapping: HashMap<Ident, TId>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_columns: HashMap<TId, TableColumns>,

    /// A buffer to be added into current pipeline
    pipeline: Vec<Transform>,
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
            input_mapping: HashMap::new(),
            table_mapping: HashMap::new(),
            table_columns: HashMap::new(),

            pipeline: Vec::new(),
        }
    }

    fn lower_table_ref(&mut self, expr: Expr) -> Result<rq::TableRef> {
        log::debug!(
            "lowering an instance of table {expr} (id={})...",
            expr.id.unwrap()
        );

        let fq_table = expr.kind.into_ident().unwrap();
        let id = self.ensure_table_id(&fq_table);
        let alias = expr.alias.clone();

        // create instance columns from table columns
        let mut columns = Vec::new();
        let mut cids_by_name = HashMap::new();
        for (name, cid) in &self.table_columns[&id] {
            let new_cid = self.cid.gen();
            cids_by_name.insert(name.clone(), new_cid);

            let kind = if name == "*" {
                ColumnDefKind::Wildcard
            } else {
                ColumnDefKind::Expr {
                    name: Some(name.clone()),
                    expr: rq::Expr {
                        kind: rq::ExprKind::ColumnRef(*cid),
                        span: None,
                    },
                }
            };
            columns.push(ColumnDecl {
                id: new_cid,
                kind,
                window: None,
                is_aggregation: false,
            });
        }

        log::debug!("... columns = {:?}", cids_by_name);
        self.input_mapping.insert(expr.id.unwrap(), cids_by_name);

        Ok(rq::TableRef {
            source: id,
            name: alias,
            columns,
        })
    }

    fn ensure_table_id(&mut self, fq_ident: &Ident) -> TId {
        *self
            .table_mapping
            .entry(fq_ident.clone())
            .or_insert_with(|| self.tid.gen())
    }

    fn lower_relation(&mut self, expr: Expr) -> Result<(rq::Relation, TableColumns)> {
        let ty = expr.ty.clone();

        let mut transforms = self.lower_pipeline(expr)?;
        let cols = self.push_select(ty, &mut transforms)?;

        Ok((rq::Relation::Pipeline(transforms), cols))
    }

    fn lower_pipeline(&mut self, ast: pl::Expr) -> Result<Vec<Transform>> {
        let mut transform_call = match ast.kind {
            pl::ExprKind::TransformCall(transform) => transform,
            _ => {
                bail!(Error::new(Reason::Expected {
                    who: None,
                    expected: "pipeline that resolves to a table".to_string(),
                    found: format!("`{ast}`")
                })
                .with_help("are you missing `from` statement?")
                .with_span(ast.span))
            }
        };

        self.pipeline.clear();

        // results starts with result of inner table
        if let Some(tbl) = transform_call.kind.tbl_arg_mut().cloned() {
            let pipeline = self.lower_pipeline(tbl)?;
            self.pipeline.extend(pipeline);
        }

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
            pl::TransformKind::From(expr) => {
                let id = self.lower_table_ref(expr)?;

                self.pipeline.push(Transform::From(id));
            }
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

        Ok(self.pipeline.drain(..).collect_vec())
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
        let frame = ty.unwrap().into_table().unwrap();

        log::debug!("push_select of a frame: {:?}", frame);

        let mut columns = Vec::new();
        let mut in_wildcards = HashSet::new();

        // wildcards
        for col in &frame.columns {
            if let FrameColumn::Wildcard { input_name } = col {
                let input = frame.find_input(input_name).unwrap();
                let input_cols = &self.input_mapping[&input.id];

                for (name, cid) in input_cols {
                    in_wildcards.insert(cid);
                    columns.push((Some(name.clone()), *cid));
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
        let has_alias = expr_ast.alias.is_some();
        let needs_window = expr_ast.needs_window;
        expr_ast.needs_window = false;
        let name = if let Some(alias) = expr_ast.alias.clone() {
            Some(alias)
        } else {
            expr_ast.kind.as_ident().map(|x| x.name.clone())
        };
        let id = expr_ast.id.unwrap();

        // lower
        let expr = self.lower_expr(expr_ast)?;

        // don't create new ColumnDef if expr is just a ColumnRef
        if let rq::ExprKind::ColumnRef(cid) = &expr.kind {
            if !has_alias && !needs_window {
                self.column_mapping.insert(id, *cid);
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
            kind: ColumnDefKind::Expr { name, expr },
            window,
            is_aggregation,
        };
        self.column_mapping.insert(id, cid);

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
        Ok(if let Some(cid) = self.column_mapping.get(&id).cloned() {
            cid
        } else if let Some(input) = self.input_mapping.get(&id) {
            let name = match name {
                Some(v) => v,
                None => bail!(Error::new(Reason::Simple(
                    "This table contains unnamed columns, that need to be referenced by name"
                        .to_string()
                ))
                .with_span(self.context.span_map.get(&id).cloned())),
            };
            log::trace!("lookup cid of name={name:?} in input {input:?}");

            if let Some(cid) = input.get(name).or_else(|| input.get("*")) {
                *cid
            } else {
                panic!("cannot find cid by id={id} and name={name:?}");
            }
        } else {
            panic!("cannot find cid by id={id}");
        })
    }
}

// Collects all ExternRefs and
#[derive(Default)]
struct TableExtractor {
    path: Vec<String>,

    tables_extern: Vec<(Ident, DeclKind)>,
    tables_pipeline: Vec<(Ident, DeclKind)>,
}

impl TableExtractor {
    fn extract(lowerer: &mut Lowerer) -> Result<Vec<TableDecl>> {
        let mut te = TableExtractor::default();

        te.extract_from_namespace(&lowerer.context.root_mod);

        // TODO: this sorts tables by names, just to make compiler output stable
        // ideally, they would preserve order in the PRQL query or use toposort
        te.tables_pipeline.sort_by_key(|(i, _)| i.name.clone());

        (te.tables_extern.into_iter())
            .chain(te.tables_pipeline)
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
                DeclKind::TableDef { expr, .. } => {
                    let fq_ident = Ident::from_path(self.path.clone());
                    let table = (fq_ident, entry.kind.clone());
                    if expr.is_none() {
                        self.tables_extern.push(table);
                    } else {
                        self.tables_pipeline.push(table);
                    }
                }
                _ => {}
            }
            self.path.pop();
        }
    }
}

fn lower_table(lowerer: &mut Lowerer, table: DeclKind, fq_ident: Ident) -> Result<TableDecl> {
    let id = lowerer.ensure_table_id(&fq_ident);

    let (frame, expr) = table.into_table_def().unwrap();

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

    // if let Declaration::Column { table, column } = decl {
    //     // yes, this CId could have been generated only if needed
    //     // but I don't want to bother with lowerer mut borrow
    //     let new_cid = self.lowerer.cid.gen();
    //     let kind = ColumnDefKind::ExternRef(column.clone());
    //     let col_def = ColumnDef {
    //         id: new_cid,
    //         kind,
    //         window: None,
    //     };

    //     let (_, cols) = self.lowerer.extern_table_entry(table);
    //     let existing = cols.iter().find_map(|cd| match &cd.kind {
    //         ColumnDefKind::ExternRef(name) if *name == column => Some(cd.id),
    //         _ => None,
    //     });
    //     if let Some(existing) = existing {
    //         self.lowerer.column_mapping.insert(id, existing);
    //     } else {
    //         cols.push(col_def);
    //         self.lowerer.column_mapping.insert(id, new_cid);
    //     }
    // }
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
                TableColumn::Wildcard => ColumnDefKind::Wildcard,
                TableColumn::Single(name) => ColumnDefKind::ExternRef(name.clone().unwrap()),
            },
            window: None,
            is_aggregation: false,
        })
        .collect_vec();

    let cols = column_defs
        .iter()
        .map(|cd| match &cd.kind {
            ColumnDefKind::Wildcard => ("*".to_string(), cd.id),
            ColumnDefKind::ExternRef(name) => (name.clone(), cd.id),
            ColumnDefKind::Expr { .. } => unreachable!(),
        })
        .collect();
    let expr = rq::Relation::ExternRef(
        TableExternRef::LocalTable(fq_ident.name.clone()),
        column_defs,
    );
    (expr, cols)
}
