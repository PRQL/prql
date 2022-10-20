use std::collections::HashMap;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::{Expr, InterpolateItem, Range};
use crate::error::{Error, Reason};
use crate::ir::{CId, ColumnDef, IdGenerator, Query, TId, Table, TableExpr, Transform};
use crate::{ast, ir};

use super::Context;

/// Convert AST into IR and make sure that:
/// - transforms are not nested,
/// - transforms have correct partition, window and sort set,
/// - make sure there are no unresolved
pub fn lower_ast_to_ir(statements: Vec<ast::Stmt>, context: Context) -> Result<Query> {
    let mut l = Lowerer::new(context);

    let mut query_def = None;
    let mut main_pipeline = None;
    let mut tables = Vec::new();

    for statement in statements {
        match statement.kind {
            ast::StmtKind::QueryDef(def) => query_def = Some(def),
            ast::StmtKind::FuncDef(_) => {}
            ast::StmtKind::TableDef(table_def) => {
                let tid = l.ensure_table_id(table_def.id.unwrap());

                tables.push(Table {
                    id: tid,
                    name: Some(table_def.name),
                    expr: l.lower_table_expr(*table_def.value)?,
                });
            }
            ast::StmtKind::Pipeline(expr) => {
                let ir = l.lower_table_expr(expr)?;
                main_pipeline = Some(ir);
            }
        }
    }

    tables.extend(l.tables);

    Ok(Query {
        def: query_def.unwrap_or_default(),
        tables,
        expr: main_pipeline
            .ok_or_else(|| Error::new(Reason::Simple("missing main pipeline".to_string())))?,
    })
}

struct Lowerer {
    ids: IdGenerator,

    context: Context,

    /// mapping from [crate::semantic::Declarations] into [CId]s
    column_mapping: HashMap<usize, CId>,

    /// mapping from [crate::semantic::Declarations] into [TId]s
    table_mapping: HashMap<usize, TId>,

    /// tables to be added to Query.tables
    tables: Vec<Table>,
}

impl Lowerer {
    fn new(context: Context) -> Self {
        Lowerer {
            context,

            ids: IdGenerator::empty(),
            column_mapping: HashMap::new(),
            table_mapping: HashMap::new(),
            tables: Vec::new(),
        }
    }

    fn lower_table(&mut self, expr: Expr) -> Result<TId> {
        let id = self.ensure_table_id(expr.declared_at.unwrap());

        let expr = self.lower_table_expr(expr)?;

        let name = match &expr {
            TableExpr::Ref(ast::TableRef::LocalTable(name)) => Some(name.clone()),
            _ => None,
        };

        self.tables.push(Table { id, name, expr });

        Ok(id)
    }

    fn ensure_table_id(&mut self, id: usize) -> TId {
        *self
            .table_mapping
            .entry(id)
            .or_insert_with(|| self.ids.gen_tid())
    }

    fn lower_table_expr(&mut self, expr: Expr) -> Result<TableExpr> {
        Ok(match expr.kind {
            ast::ExprKind::Ident(name) => TableExpr::Ref(ast::TableRef::LocalTable(name)),
            _ => TableExpr::Pipeline(self.lower_transform(expr)?),
        })
    }

    fn lower_transform(&mut self, ast: ast::Expr) -> Result<Vec<Transform>> {
        let transform_call = match ast.kind {
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

        let tbl = match *transform_call.kind {
            ast::TransformKind::From(expr) => {
                let id = self.lower_table(expr)?;

                transforms.push(Transform::From(id));

                None
            }
            ast::TransformKind::Derive { assigns, tbl } => {
                for assign in assigns {
                    self.declare_as_column(assign, &mut transforms)?;
                }

                Some(tbl)
            }
            ast::TransformKind::Select { assigns, tbl } => {
                let mut select = Vec::new();
                for assign in assigns {
                    let iid = self.declare_as_column(assign, &mut transforms)?;
                    select.push(iid);
                }
                transforms.push(Transform::Select(select));

                Some(tbl)
            }
            ast::TransformKind::Filter { filter, tbl } => {
                transforms.push(Transform::Filter(self.lower_expr(filter)?));

                Some(tbl)
            }
            ast::TransformKind::Aggregate { assigns, tbl } => {
                let mut select = Vec::new();
                for assign in assigns {
                    let name = assign.alias.clone();
                    let expr = self.lower_expr(assign)?;

                    let id = self.ids.gen_cid();
                    let def = ColumnDef { id, expr, name };
                    select.push(def);
                }

                transforms.push(Transform::Aggregate(select));
                Some(tbl)
            }
            ast::TransformKind::Sort { by, tbl } => {
                let mut sorts = Vec::new();
                for ast::ColumnSort { column, direction } in by {
                    let column = self.declare_as_column(column, &mut transforms)?;
                    sorts.push(ast::ColumnSort { direction, column });
                }
                transforms.push(Transform::Sort(sorts));

                Some(tbl)
            }
            ast::TransformKind::Take { range, tbl } => {
                let range = Range {
                    start: range.start.map(|x| self.lower_expr(*x)).transpose()?,
                    end: range.end.map(|x| self.lower_expr(*x)).transpose()?,
                };

                transforms.push(Transform::Take(range));

                Some(tbl)
            }
            ast::TransformKind::Join {
                side,
                with,
                filter,
                tbl,
            } => {
                let with = self.lower_table(with)?;

                let transform = Transform::Join {
                    side,
                    with,
                    filter: match filter {
                        ast::JoinFilter::On(exprs) => {
                            ast::JoinFilter::On(self.declare_as_columns(exprs, &mut transforms)?)
                        }
                        ast::JoinFilter::Using(exprs) => {
                            ast::JoinFilter::Using(self.declare_as_columns(exprs, &mut transforms)?)
                        }
                    },
                };
                transforms.push(transform);

                Some(tbl)
            }
            ast::TransformKind::Group { by, pipeline, tbl } => {
                let mut partition = Vec::new();
                for x in by {
                    let iid = self.declare_as_column(x, &mut transforms)?;
                    partition.push(iid);
                }

                transforms.extend(self.lower_transform(pipeline)?);

                Some(tbl)
            }
            ast::TransformKind::Window { tbl, .. } => Some(tbl),
        };

        // results starts with result of inner table
        let mut result = if let Some(tbl) = tbl {
            self.lower_transform(tbl)?
        } else {
            Vec::new()
        };

        // ... and continues with transforms created in this function
        result.extend(transforms);

        Ok(result)
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
        let name = expr_ast.alias.clone();
        let id = expr_ast.declared_at;

        let expr = self.lower_expr(expr_ast)?;

        let cid = self.ids.gen_cid();
        let def = ColumnDef {
            id: cid,
            expr,
            name,
        };

        if let Some(id) = id {
            self.column_mapping.insert(id, cid);
        }

        transforms.push(Transform::Derive(def));
        Ok(cid)
    }

    fn lower_expr(&mut self, ast: ast::Expr) -> Result<ir::Expr> {
        // this should be refactored:
        // - ident contains some important decl lookups,
        // - while SString and FString just fold the tree.

        let kind = match ast.kind {
            ast::ExprKind::Ident(_) => {
                let id = ast.declared_at.unwrap();
                let decl = self.context.declarations.get(id);

                match decl {
                    super::Declaration::Expression(expr) => {
                        if let Some(cid) = self.column_mapping.get(&id).cloned() {
                            ir::ExprKind::ColumnRef(cid)
                        } else {
                            self.lower_expr(*expr.clone())?.kind
                        }
                    }
                    super::Declaration::ExternRef { table, variable } => ir::ExprKind::ExternRef {
                        variable: variable.clone(),
                        table: table.map(|x| self.ensure_table_id(x)),
                    },
                    super::Declaration::Table(_) => bail!("Cannot lower a table ref to IR expr"),
                    super::Declaration::Function(_) => {
                        bail!("Cannot lower a function ref to IR expr")
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
                op,
                expr: Box::new(self.lower_expr(*expr)?),
            },
            ast::ExprKind::FuncCall(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::Closure(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::TransformCall(_) => bail!("Cannot lower to IR expr: `{ast:?}`"),
            ast::ExprKind::SString(items) => ir::ExprKind::SString(
                items
                    .into_iter()
                    .map(|i| -> Result<InterpolateItem<ir::Expr>> {
                        Ok(match i {
                            InterpolateItem::String(s) => InterpolateItem::String(s),
                            InterpolateItem::Expr(e) => {
                                InterpolateItem::Expr(Box::new(self.lower_expr(*e)?))
                            }
                        })
                    })
                    .try_collect()?,
            ),
            ast::ExprKind::FString(items) => ir::ExprKind::SString(
                items
                    .into_iter()
                    .map(|i| -> Result<InterpolateItem<ir::Expr>> {
                        Ok(match i {
                            InterpolateItem::String(s) => InterpolateItem::String(s),
                            InterpolateItem::Expr(e) => {
                                InterpolateItem::Expr(Box::new(self.lower_expr(*e)?))
                            }
                        })
                    })
                    .try_collect()?,
            ),
        };

        Ok(ir::Expr {
            kind,
            span: ast.span,
        })
    }
}
