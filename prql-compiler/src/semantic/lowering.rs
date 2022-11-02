use std::collections::HashMap;

use anyhow::{bail, Result};
use itertools::Itertools;

use crate::ast::{Expr, InterpolateItem, Range};
use crate::error::{Error, Reason};
use crate::ir::{
    CId, ColumnDef, ColumnDefKind, IdGenerator, Query, TId, Table, TableExpr, Transform,
};
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

    for statement in statements {
        match statement.kind {
            ast::StmtKind::QueryDef(def) => query_def = Some(def),
            ast::StmtKind::FuncDef(_) => {}
            ast::StmtKind::TableDef(table_def) => {
                let id = l.ensure_table_id(table_def.id.unwrap());

                let name = Some(table_def.name);
                let expr = l.lower_table_expr(*table_def.value)?;
                l.push_table(Table { id, name, expr });
            }
            ast::StmtKind::Pipeline(expr) => {
                let ir = l.lower_table_expr(expr)?;
                main_pipeline = Some(ir);
            }
        }
    }

    // set ExternRefs of Ref tables
    let tables = l
        .tables
        .into_iter()
        .map(|mut t| {
            if let TableExpr::Ref(_, cols) = &mut t.expr {
                cols.extend(l.extern_refs.remove(&t.id).unwrap_or_default());
            };
            t
        })
        .collect();

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

    /// descriptor of known table columns (this will probably go into semantic::Context)
    tables_frames: HashMap<TId, Vec<CId>>,

    /// column definitions of input tables to the query
    // This is needed here, because extern refs may be found in the query
    // even after a table is lowered.
    extern_refs: HashMap<TId, Vec<ColumnDef>>,

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
            tables_frames: HashMap::new(),
            extern_refs: HashMap::new(),
            tables: Vec::new(),
        }
    }

    fn push_table(&mut self, table: Table) {
        let columns = match &table.expr {
            TableExpr::Ref(_, cols) => cols.iter().map(|c| c.id).collect(),
            TableExpr::Pipeline(transforms) => match transforms.last() {
                Some(Transform::Select(cols)) => cols.clone(),
                _ => unreachable!(),
            },
        };

        self.tables_frames.insert(table.id, columns);
        self.tables.push(table);
    }

    fn lower_table(&mut self, expr: Expr) -> Result<TId> {
        let id = self.ensure_table_id(expr.declared_at.unwrap());

        let expr = self.lower_table_expr(expr)?;

        let name = match &expr {
            TableExpr::Ref(ast::TableRef::LocalTable(name), _) => Some(name.clone()),
            _ => None,
        };

        self.push_table(Table { id, name, expr });

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
            ast::ExprKind::Ident(name) => {
                // a table reference by name, lower to local table

                let star_col = ColumnDef {
                    id: self.ids.gen_cid(),
                    kind: ColumnDefKind::Wildcard(self.table_mapping[&expr.declared_at.unwrap()]),
                };

                TableExpr::Ref(ast::TableRef::LocalTable(name.to_string()), vec![star_col])
            }

            _ => {
                let ty = expr.ty.clone();

                let mut transforms = self.lower_transform(expr)?;
                self.push_select(ty, &mut transforms);

                TableExpr::Pipeline(transforms)
            }
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
                transforms.push(Transform::Filter(self.lower_expr(*filter)?));

                Some(tbl)
            }
            ast::TransformKind::Aggregate { assigns, tbl } => {
                let select = self.declare_as_columns(assigns, &mut transforms)?;

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
                let with = self.lower_table(*with)?;

                let transform = Transform::Join {
                    side,
                    with,
                    filter: self.lower_expr(*filter)?,
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

                transforms.extend(self.lower_transform(*pipeline)?);

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

    fn push_select(&mut self, ty: Option<ast::Ty>, transforms: &mut Vec<Transform>) {
        let frame = ty.unwrap().into_table().unwrap();

        use ast::FrameColumn::*;
        let columns = (frame.columns.into_iter())
            .flat_map(|col| match col {
                All(table_id) => {
                    let tid = self.table_mapping[&table_id];
                    self.tables_frames[&tid].clone()
                }
                Unnamed(id) | Named(_, id) => {
                    vec![self.column_mapping[&id]]
                }
            })
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
        let name = if let Some(alias) = expr_ast.alias.clone() {
            Some(alias)
        } else {
            expr_ast.kind.as_ident().map(|x| x.name.clone())
        };
        let id = expr_ast.declared_at;

        let expr = self.lower_expr(expr_ast)?;

        let cid = self.ids.gen_cid();
        let def = ColumnDef {
            id: cid,
            kind: ColumnDefKind::Expr { name, expr },
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
                    ir::ExprKind::ColumnRef(cid)
                } else {
                    let decl = self.context.declarations.get(id).clone();

                    match decl {
                        super::Declaration::Expression(expr) => self.lower_expr(*expr)?.kind,
                        super::Declaration::ExternRef { table, variable } => {
                            if let Some(table_id) = table {
                                let tid = self.ensure_table_id(table_id);
                                let refs = self.extern_refs.entry(tid).or_default();

                                let cid = self.ids.gen_cid();

                                // println!("{tid:?}: {refs:?} + {id:?}");
                                refs.push(ColumnDef {
                                    id: cid,
                                    kind: ColumnDefKind::ExternRef(variable),
                                });

                                self.column_mapping.insert(id, cid);
                                ir::ExprKind::ColumnRef(cid)
                            } else {
                                ir::ExprKind::SString(vec![InterpolateItem::String(variable)])
                            }
                        }
                        super::Declaration::Table(_) => {
                            bail!("Cannot lower a table ref to IR expr")
                        }
                        super::Declaration::Function(_) => {
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
    ) -> Result<Vec<InterpolateItem<ir::Expr>>, anyhow::Error> {
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
// pub struct ExternRefExtractor<'a> {
//     context: &'a mut AnchorContext,
// }

// impl<'a> ExternRefExtractor<'a> {
//     pub fn extract(context: &mut AnchorContext, pipeline: Vec<Transform>) -> Vec<Transform> {
//         let mut e = ExternRefExtractor { context };
//         e.fold_transforms(pipeline).unwrap()
//     }
// }

// impl<'a> IrFold for ExternRefExtractor<'a> {
//     fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
//         Ok(if let ExprKind::ExternRef { variable, .. } = &expr.kind {
//             let id = self.context.ids.gen_cid();
//             let new_def = ColumnDef {
//                 id,
//                 kind: ColumnDefKind::Expr {
//                     name: Some(variable.clone()),
//                     expr,
//                 },
//             };
//             self.context.columns_defs.insert(id, new_def);
//             Expr {
//                 kind: ExprKind::ColumnRef(id),
//                 span: None,
//             }
//         } else {
//             expr.kind = self.fold_expr_kind(expr.kind)?;
//             expr
//         })
//     }

//     fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
//         // exactly the same as default impl,
//         // except that fold_expr is inlined for the first level,
//         // to prevent extracting ExternRefs that are already in root of a def.

//         Ok(ColumnDef {
//             id: cd.id,
//             kind: match cd.kind {
//                 ColumnDefKind::Wildcard(tid) => ColumnDefKind::Wildcard(tid),
//                 ColumnDefKind::Expr { name, expr } => ColumnDefKind::Expr {
//                     name,
//                     expr: {
//                         let mut expr = expr;
//                         expr.kind = self.fold_expr_kind(expr.kind)?;
//                         expr
//                     },
//                 },
//             },
//         })
//     }
// }
