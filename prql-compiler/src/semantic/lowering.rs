use std::collections::{HashMap, HashSet};

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::{Expr, FrameColumn, Ident, InterpolateItem, Range, TableExternRef, WindowFrame};
use crate::error::{Error, Reason};
use crate::ir::{
    CId, ColumnDef, ColumnDefKind, IdGenerator, Query, TId, TableDef, TableExpr, Transform,
};
use crate::semantic::module::Module;
use crate::{ast, ir};

use super::context::{Context, DeclKind, TableColumn, TableFrame};

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved
pub fn lower_ast_to_ir(statements: Vec<ast::Stmt>, context: Context) -> Result<Query> {
    let mut l = Lowerer::new(context);

    // TODO: when extern refs will be resolved to a local instance of a table
    // instead of a the global table definition, this could be removed
    let tables = TableExtractor::extract(&mut l)?;

    let mut query_def = None;
    let mut main_pipeline = None;

    for statement in statements {
        match statement.kind {
            ast::StmtKind::QueryDef(def) => query_def = Some(def),
            ast::StmtKind::Pipeline(expr) => {
                let (ir, _) = l.lower_table_expr(*expr)?;
                main_pipeline = Some(ir);
            }
            ast::StmtKind::FuncDef(_) | ast::StmtKind::TableDef(_) => {}
        }
    }

    Ok(Query {
        def: query_def.unwrap_or_default(),
        tables,
        expr: main_pipeline
            .ok_or_else(|| Error::new(Reason::Simple("missing main pipeline".to_string())))?,
    })
}

struct Lowerer {
    cid: IdGenerator<CId>,
    tid: IdGenerator<TId>,

    decls: Context,

    // current window for any new column defs
    window: Option<ir::Window>,

    /// mapping from [Expr].id into [CId]s
    column_mapping: HashMap<usize, CId>,

    input_mapping: HashMap<usize, HashMap<String, CId>>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_mapping: HashMap<Ident, TId>,

    /// mapping from [Ident] of [crate::ast::TableDef] into [TId]s
    table_columns: HashMap<TId, TableColumns>,
}

type TableColumns = Vec<(String, CId)>;

impl Lowerer {
    fn new(context: Context) -> Self {
        Lowerer {
            decls: context,
            window: None,

            cid: IdGenerator::new(),
            tid: IdGenerator::new(),

            column_mapping: HashMap::new(),
            input_mapping: HashMap::new(),
            table_mapping: HashMap::new(),
            table_columns: HashMap::new(),
        }
    }

    fn lower_table_ref(&mut self, expr: Expr) -> Result<ir::TableRef> {
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
                    expr: ir::Expr {
                        kind: ir::ExprKind::ColumnRef(*cid),
                        span: None,
                    },
                }
            };
            columns.push(ColumnDef {
                id: new_cid,
                kind,
                window: None,
            });
        }

        log::debug!("... columns = {:?}", cids_by_name);
        self.input_mapping.insert(expr.id.unwrap(), cids_by_name);

        Ok(ir::TableRef {
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

    fn lower_table_expr(&mut self, expr: Expr) -> Result<(TableExpr, TableColumns)> {
        let ty = expr.ty.clone();

        let mut transforms = self.lower_transform(expr)?;
        let cols = self.push_select(ty, &mut transforms);

        Ok((TableExpr::Pipeline(transforms), cols))
    }

    fn lower_transform(&mut self, ast: ast::Expr) -> Result<Vec<Transform>> {
        let mut transform_call = match ast.kind {
            ast::ExprKind::TransformCall(transform) => transform,
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

        let mut transforms = Vec::new();

        // results starts with result of inner table
        if let Some(tbl) = transform_call.kind.tbl_arg_mut().cloned() {
            transforms.extend(self.lower_transform(tbl)?);
        }

        // ... and continues with transforms created in this function

        let window = ir::Window {
            frame: WindowFrame {
                kind: transform_call.frame.kind,
                range: self.lower_range(transform_call.frame.range)?,
            },
            partition: self.declare_as_columns(transform_call.partition, &mut transforms)?,
            sort: self.lower_sorts(transform_call.sort, &mut transforms)?,
        };
        self.window = Some(window);

        match *transform_call.kind {
            ast::TransformKind::From(expr) => {
                let id = self.lower_table_ref(expr)?;

                transforms.push(Transform::From(id));
            }
            ast::TransformKind::Derive { assigns, .. } => {
                for assign in assigns {
                    self.declare_as_column(assign, &mut transforms)?;
                }
            }
            ast::TransformKind::Select { assigns, .. } => {
                let mut select = Vec::new();
                for assign in assigns {
                    let iid = self.declare_as_column(assign, &mut transforms)?;
                    select.push(iid);
                }
                transforms.push(Transform::Select(select));
            }
            ast::TransformKind::Filter { filter, .. } => {
                transforms.push(Transform::Filter(self.lower_expr(*filter)?));
            }
            ast::TransformKind::Aggregate { assigns, .. } => {
                let window = self.window.take();

                let select = self.declare_as_columns(assigns, &mut transforms)?;
                transforms.push(Transform::Select(select));

                let by = window.unwrap().partition;
                transforms.push(Transform::Aggregate { by });
            }
            ast::TransformKind::Sort { by, .. } => {
                let sorts = self.lower_sorts(by, &mut transforms)?;
                transforms.push(Transform::Sort(sorts));
            }
            ast::TransformKind::Take { range, .. } => {
                let range = Range {
                    start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
                    end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
                };

                transforms.push(Transform::Take(range));
            }
            ast::TransformKind::Join {
                side, with, filter, ..
            } => {
                let with = self.lower_table_ref(*with)?;

                let transform = Transform::Join {
                    side,
                    with,
                    filter: self.lower_expr(*filter)?,
                };
                transforms.push(transform);
            }
            ast::TransformKind::Group { .. } | ast::TransformKind::Window { .. } => unreachable!(
                "transform `{}` cannot be lowered.",
                (*transform_call.kind).as_ref()
            ),
        }
        self.window = None;

        Ok(transforms)
    }

    fn lower_range(&mut self, range: ast::Range<Box<ast::Expr>>) -> Result<Range<ir::Expr>> {
        Ok(Range {
            start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
            end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
        })
    }

    fn lower_sorts(
        &mut self,
        by: Vec<ast::ColumnSort>,
        transforms: &mut Vec<Transform>,
    ) -> Result<Vec<ast::ColumnSort<CId>>> {
        by.into_iter()
            .map(|ast::ColumnSort { column, direction }| {
                let column = self.declare_as_column(column, transforms)?;
                Ok(ast::ColumnSort { direction, column })
            })
            .try_collect()
    }

    /// Append a Select of final table columns derived from frame
    fn push_select(
        &mut self,
        ty: Option<ast::Ty>,
        transforms: &mut Vec<Transform>,
    ) -> TableColumns {
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
                let cid = self.lookup_cid(*expr_id, name.as_ref());

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

        names
    }

    fn declare_as_columns(
        &mut self,
        exprs: Vec<ast::Expr>,
        transforms: &mut Vec<Transform>,
    ) -> Result<Vec<CId>> {
        exprs
            .into_iter()
            .map(|x| self.declare_as_column(x, transforms))
            .try_collect()
    }

    fn declare_as_column(
        &mut self,
        expr_ast: ast::Expr,
        transforms: &mut Vec<Transform>,
    ) -> Result<ir::CId> {
        // copy metadata before lowering
        let has_alias = expr_ast.alias.is_some();
        let needs_window = expr_ast.needs_window;
        let name = if let Some(alias) = expr_ast.alias.clone() {
            Some(alias)
        } else {
            expr_ast.kind.as_ident().map(|x| x.name.clone())
        };
        let id = expr_ast.id.unwrap();

        // lower
        let expr = self.lower_expr(expr_ast)?;

        // don't create new ColumnDef if expr is just a ColumnRef
        if let ir::ExprKind::ColumnRef(cid) = &expr.kind {
            if !has_alias && !needs_window {
                self.column_mapping.insert(id, *cid);
                return Ok(*cid);
            }
        }

        // determine window, but only for s-strings
        let window = if needs_window {
            self.window.clone()
        } else {
            None
        };

        // construct ColumnDef
        let cid = self.cid.gen();
        let def = ColumnDef {
            id: cid,
            kind: ColumnDefKind::Expr { name, expr },
            window,
        };
        self.column_mapping.insert(id, cid);

        transforms.push(Transform::Compute(def));
        Ok(cid)
    }

    fn lower_expr(&mut self, ast: ast::Expr) -> Result<ir::Expr> {
        let kind = match ast.kind {
            ast::ExprKind::Ident(ident) => {
                log::debug!("lowering ident {ident} (target {:?})", ast.target_id);

                if let Some(id) = ast.target_id {
                    let cid = self.lookup_cid(id, Some(&ident.name));

                    ir::ExprKind::ColumnRef(cid)
                } else {
                    // This is an unresolved ident.
                    // Let's hope that the database engine can resolve it.
                    ir::ExprKind::SString(vec![InterpolateItem::String(ident.name)])
                }
            }
            ast::ExprKind::Literal(literal) => ir::ExprKind::Literal(literal),
            ast::ExprKind::Pipeline(_) => bail!("Cannot lower AST that has not been resolved"),
            ast::ExprKind::List(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::Range(Range { start, end }) => ir::ExprKind::Range(Range {
                start: start
                    .map(|x| self.lower_expr(*x))
                    .transpose()?
                    .map(Box::new),
                end: end.map(|x| self.lower_expr(*x)).transpose()?.map(Box::new),
            }),
            ast::ExprKind::Binary { left, op, right } => ir::ExprKind::Binary {
                left: Box::new(self.lower_expr(*left)?),
                op,
                right: Box::new(self.lower_expr(*right)?),
            },
            ast::ExprKind::Unary { op, expr } => ir::ExprKind::Unary {
                op: match op {
                    ast::UnOp::Neg => ir::UnOp::Neg,
                    ast::UnOp::Not => ir::UnOp::Not,
                    ast::UnOp::EqSelf => bail!("Cannot lower to IR expr: `{op:?}`"),
                },
                expr: Box::new(self.lower_expr(*expr)?),
            },
            ast::ExprKind::FuncCall(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::Closure(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::TransformCall(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::SString(items) => {
                ir::ExprKind::SString(self.lower_interpolations(items)?)
            }
            ast::ExprKind::FString(items) => {
                ir::ExprKind::FString(self.lower_interpolations(items)?)
            }
        };

        Ok(ir::Expr {
            kind,
            span: ast.span,
        })
    }

    fn lower_interpolations(
        &mut self,
        items: Vec<InterpolateItem>,
    ) -> Result<Vec<InterpolateItem<ir::Expr>>> {
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

    fn lookup_cid(&self, id: usize, name: Option<&String>) -> CId {
        if let Some(cid) = self.column_mapping.get(&id).cloned() {
            cid
        } else if let Some(input) = self.input_mapping.get(&id) {
            let name = name.unwrap();
            log::debug!("lookup cid of name={name:?} in input {input:?}");

            if let Some(cid) = input.get(name).or_else(|| input.get("*")) {
                *cid
            } else {
                panic!("cannot find cid by id={id} and name={name:?}");
            }
        } else {
            panic!("cannot find cid by id={id}");
        }
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
    fn extract(lowerer: &mut Lowerer) -> Result<Vec<TableDef>> {
        let mut te = TableExtractor::default();

        te.extract_from_namespace(&lowerer.decls.root_mod);

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

fn lower_table(lowerer: &mut Lowerer, table: DeclKind, fq_ident: Ident) -> Result<TableDef> {
    let id = lowerer.ensure_table_id(&fq_ident);

    let (frame, expr) = table.into_table_def().unwrap();

    let (expr, cols) = if let Some(expr) = expr {
        // this is a CTE
        lowerer.lower_table_expr(*expr)?
    } else {
        lower_extern_table(lowerer, frame, &fq_ident)
    };
    let name = Some(fq_ident.name.clone());

    log::debug!("lowering table {name:?}, columns = {:?}", cols);
    lowerer.table_columns.insert(id, cols);

    Ok(TableDef { id, name, expr })

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
) -> (TableExpr, TableColumns) {
    let column_defs = (frame.columns.iter())
        .map(|col| ColumnDef {
            id: lowerer.cid.gen(),
            kind: match col {
                TableColumn::Wildcard => ColumnDefKind::Wildcard,
                TableColumn::Single(name) => ColumnDefKind::ExternRef(name.clone().unwrap()),
            },
            window: None,
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
    let expr = TableExpr::ExternRef(
        TableExternRef::LocalTable(fq_ident.name.clone()),
        column_defs,
    );
    (expr, cols)
}
