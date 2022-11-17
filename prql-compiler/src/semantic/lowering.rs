use std::collections::HashMap;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::AstFold;
use crate::ast::{Expr, InterpolateItem, Range, TableExternRef, WindowFrame};
use crate::error::{Error, Reason};
use crate::ir::{
    CId, ColumnDef, ColumnDefKind, IdGenerator, Query, TId, TableDef, TableExpr, Transform,
};
use crate::{ast, ir};

use super::{Context, Declaration};

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved
pub fn lower_ast_to_ir(statements: Vec<ast::Stmt>, context: Context) -> Result<Query> {
    let mut l = Lowerer::new(context);

    // TODO: when extern refs will be resolved to a local instance of a table
    // instead of a the global table definition, this could be removed
    let statements = ExternRefExtractor::extract(&mut l, statements);

    let mut query_def = None;
    let mut main_pipeline = None;

    for statement in statements {
        match statement.kind {
            ast::StmtKind::QueryDef(def) => query_def = Some(def),
            ast::StmtKind::FuncDef(_) => {}
            ast::StmtKind::TableDef(table_def) => {
                let id = l.ensure_table_id(table_def.id.unwrap());

                let name = Some(table_def.name);
                let expr = l.lower_table_pipeline(*table_def.value)?;
                l.tables_pipeline.push(TableDef { id, name, expr });
            }
            ast::StmtKind::Pipeline(expr) => {
                let ir = l.lower_table_pipeline(*expr)?;
                main_pipeline = Some(ir);
            }
        }
    }

    // TODO: remove this block after proper table def inference is in place
    for t in l.tables_extern.values_mut() {
        match &mut t.expr {
            TableExpr::ExternRef(name, _) => {
                *name = TableExternRef::LocalTable(t.name.clone().unwrap());
            }
            TableExpr::Pipeline(_) => unreachable!(),
        }
    }

    let tables = (l.tables_extern.into_values())
        .chain(l.tables_pipeline.into_iter())
        .collect();
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

    context: Context,

    // current window for any new column defs
    window: Option<ir::Window>,

    /// mapping from [crate::semantic::Declarations] into [CId]s
    column_mapping: HashMap<usize, CId>,

    /// mapping from [crate::semantic::Declarations] into [TId]s
    table_mapping: HashMap<usize, TId>,

    // TODO: this is a workaround for not resolving columns to a table instance, but to underlying extern table
    cid_redirect: HashMap<CId, CId>,

    /// tables to be added to Query.tables
    tables_extern: HashMap<TId, TableDef>,

    /// tables to be added to Query.tables
    tables_pipeline: Vec<TableDef>,
}

impl Lowerer {
    fn new(context: Context) -> Self {
        Lowerer {
            context,
            window: None,

            cid: IdGenerator::new(),
            tid: IdGenerator::new(),

            column_mapping: HashMap::new(),
            table_mapping: HashMap::new(),

            cid_redirect: HashMap::new(),

            tables_extern: HashMap::new(),
            tables_pipeline: Vec::new(),
        }
    }

    fn lower_table_ref(&mut self, expr: Expr) -> Result<ir::TableRef> {
        let id = self.ensure_table_id(expr.declared_at.unwrap());
        let alias = expr.alias.clone();

        let columns = if let Some(table) = self.tables_pipeline.iter().find(|t| t.id == id) {
            if let TableExpr::Pipeline(transforms) = &table.expr {
                transforms.last().unwrap().as_select().unwrap().clone()
            } else {
                unreachable!();
            }
        } else {
            let (name, cols) = self.extern_table_entry(expr.declared_at.unwrap());

            *name = Some(expr.kind.into_ident().unwrap().name);

            cols.iter().map(|c| c.id).collect()
        };

        // create columns of the table instance
        let columns = columns
            .into_iter()
            .map(|cid| {
                let new_cid = self.cid.gen();
                self.cid_redirect.insert(cid, new_cid);
                ColumnDef {
                    id: new_cid,
                    kind: ColumnDefKind::Expr {
                        name: None,
                        expr: ir::Expr {
                            kind: ir::ExprKind::ColumnRef(cid),
                            span: None,
                        },
                    },
                    window: None,
                }
            })
            .collect();

        Ok(ir::TableRef {
            source: id,
            name: alias,
            columns,
        })
    }

    fn extern_table_entry(&mut self, id: usize) -> (&mut Option<String>, &mut Vec<ColumnDef>) {
        let tid = self.ensure_table_id(id);
        let refs = self.tables_extern.entry(tid);

        let table = refs.or_insert_with(|| TableDef {
            id: tid,
            name: None,
            expr: TableExpr::ExternRef(
                TableExternRef::LocalTable("".to_string()),
                vec![ColumnDef {
                    id: self.cid.gen(),
                    kind: ColumnDefKind::Wildcard,
                    window: None,
                }],
            ),
        });

        match &mut table.expr {
            TableExpr::ExternRef(_, cols) => (&mut table.name, cols),
            TableExpr::Pipeline(_) => unreachable!(),
        }
    }

    fn ensure_table_id(&mut self, id: usize) -> TId {
        *self
            .table_mapping
            .entry(id)
            .or_insert_with(|| self.tid.gen())
    }

    fn lower_table_pipeline(&mut self, expr: Expr) -> Result<TableExpr> {
        let ty = expr.ty.clone();

        let mut transforms = self.lower_transform(expr)?;
        self.push_select(ty, &mut transforms);

        Ok(TableExpr::Pipeline(transforms))
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
                self.window = None;
                let select = self.declare_as_columns(assigns, &mut transforms)?;

                transforms.push(Transform::Aggregate(select))
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
            ast::TransformKind::Group { .. } | ast::TransformKind::Window { .. } => unreachable!(),
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
    #[allow(clippy::needless_collect)]
    fn push_select(&mut self, ty: Option<ast::Ty>, transforms: &mut Vec<Transform>) {
        let frame = ty.unwrap().into_table().unwrap();

        log::debug!("push_select of a frame: {:?}", frame.columns);

        use ast::FrameColumn::*;
        let columns = (frame.columns.into_iter())
            .map(|col| match col {
                All(table_id) => {
                    let (_, cols) = self.extern_table_entry(table_id);
                    cols.iter()
                        .find_map(|cd| match cd.kind {
                            ColumnDefKind::Wildcard => Some(cd.id),
                            _ => None,
                        })
                        .unwrap()
                }
                Unnamed(id) | Named(_, id) => self.column_mapping[&id],
            })
            .collect::<Vec<_>>();

        let columns = columns
            .into_iter()
            .map(|cid| self.cid_redirect.get(&cid).cloned().unwrap_or(cid))
            .collect();
        transforms.push(Transform::Select(columns));
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
        let id = expr_ast.declared_at;

        // lower
        let expr = self.lower_expr(expr_ast)?;

        // don't create new ColumnDef if expr is just a ColumnRef
        if let ir::ExprKind::ColumnRef(cid) = &expr.kind {
            if !has_alias && !needs_window {
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

        if let Some(id) = id {
            self.column_mapping.insert(id, cid);
        }

        transforms.push(Transform::Compute(def));
        Ok(cid)
    }

    fn lower_expr(&mut self, ast: ast::Expr) -> Result<ir::Expr> {
        // this should be refactored:
        // - ident contains some important decl lookups,
        // - while SString and FString just fold the tree.

        let kind = match ast.kind {
            ast::ExprKind::Ident(_) => {
                let id = ast.declared_at.expect("unresolved ident node");

                if let Some(cid) = self.column_mapping.get(&id).cloned() {
                    let cid = self.cid_redirect.get(&cid).cloned().unwrap_or(cid);

                    ir::ExprKind::ColumnRef(cid)
                } else {
                    let decl = self.context.declarations.get(id).clone();

                    match decl {
                        Declaration::Expression(expr) => self.lower_expr(*expr)?.kind,
                        Declaration::ExternRef { table, variable } => {
                            if table.is_some() {
                                // extern ref has been extracted with ExternRefExtractor prior to lowering

                                let cid = self.column_mapping.get(&id).unwrap();
                                ir::ExprKind::ColumnRef(*cid)
                            } else {
                                ir::ExprKind::SString(vec![InterpolateItem::String(variable)])
                            }
                        }
                        Declaration::Table(_) => {
                            bail!("Cannot lower a table ref to IR expr")
                        }
                        Declaration::Function(_) => {
                            bail!("Cannot lower a function ref to IR expr")
                        }
                    }
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
}

// Collects all ExternRefs and
struct ExternRefExtractor<'a> {
    lowerer: &'a mut Lowerer,
}

impl<'a> ExternRefExtractor<'a> {
    fn extract(lowerer: &mut Lowerer, stmts: Vec<ast::Stmt>) -> Vec<ast::Stmt> {
        let mut e = ExternRefExtractor { lowerer };
        e.fold_stmts(stmts).unwrap()
    }
}

impl<'a> AstFold for ExternRefExtractor<'a> {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        if let Some(id) = expr.declared_at {
            let decl = self.lowerer.context.declarations.get(id).clone();

            if let Declaration::ExternRef {
                table: Some(table_id),
                variable,
            } = decl
            {
                // yes, this CId could have been generated only if needed
                // but I don't want to bother with lowerer mut borrow
                let new_cid = self.lowerer.cid.gen();
                let kind = ColumnDefKind::ExternRef(variable.clone());
                let col_def = ColumnDef {
                    id: new_cid,
                    kind,
                    window: None,
                };

                let (_, cols) = self.lowerer.extern_table_entry(table_id);
                let existing = cols.iter().find_map(|cd| match &cd.kind {
                    ColumnDefKind::ExternRef(name) if *name == variable => Some(cd.id),
                    _ => None,
                });
                if let Some(existing) = existing {
                    self.lowerer.column_mapping.insert(id, existing);
                } else {
                    cols.push(col_def);
                    self.lowerer.column_mapping.insert(id, new_cid);
                }
            }
        }

        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
}
