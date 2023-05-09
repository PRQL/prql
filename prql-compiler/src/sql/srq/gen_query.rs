//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::Result;
use itertools::Itertools;

use crate::ast::rq::{Query, RelationKind, RqFold, TableRef, Transform};
use crate::Target;

use super::anchor;
use super::ast::{fold_sql_transform, RelationExpr, SqlQuery, SqlRelation, SqlTransform, SrqFold};
use super::context::{AnchorContext, RelationAdapter, RelationStatus};

use super::super::{Context, Dialect};
use super::preprocess;

pub(in super::super) fn compile_query(
    query: Query,
    dialect: Option<Dialect>,
) -> Result<(SqlQuery, Context)> {
    let dialect = if let Some(dialect) = dialect {
        dialect
    } else {
        let target = query.def.other.get("target");
        let Target::Sql(maybe_dialect) = target
            .map(|s| Target::from_str(s))
            .transpose()?
            .unwrap_or_default();
        maybe_dialect.unwrap_or_default()
    };
    let dialect = dialect.handler();

    let (anchor, main_relation) = AnchorContext::of(query);

    let mut ctx = Context::new(dialect, anchor);

    // compile main relation that will recursively compile CTEs
    let main_relation = compile_relation(main_relation.into(), &mut ctx)?;

    // attach CTEs
    let ctes = ctx.ctes.drain(..).collect_vec();

    let query = SqlQuery {
        main_relation,
        ctes,
    };
    Ok((query, ctx))
}

fn compile_relation(relation: RelationAdapter, ctx: &mut Context) -> Result<SqlRelation> {
    log::trace!("compiling relation {relation:#?}");

    Ok(match relation {
        RelationAdapter::Rq(rel) => {
            match rel.kind {
                // base case
                RelationKind::Pipeline(pipeline) => {
                    // preprocess
                    let pipeline = preprocess::preprocess(pipeline, ctx)?;

                    // load names of output columns
                    ctx.anchor.load_names(&pipeline, rel.columns);

                    compile_pipeline(pipeline, ctx)?
                }

                RelationKind::Literal(lit) => SqlRelation::Literal(lit),
                RelationKind::SString(items) => SqlRelation::SString(items),

                // ref cannot be converted directly into query and does not need it's own CTE
                RelationKind::ExternRef(_) => unreachable!(),
            }
        }

        RelationAdapter::Preprocessed(pipeline, columns) => {
            // load names of output columns
            ctx.anchor.load_names(&pipeline, columns);

            compile_pipeline(pipeline, ctx)?
        }

        RelationAdapter::Srq(rel) => rel,
    })
}

fn compile_pipeline(
    pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<SqlRelation> {
    use SqlTransform::*;

    // special case: loop
    if pipeline.iter().any(|t| matches!(t, Loop(_))) {
        todo!();
        // pipeline = sql_of_loop(pipeline, ctx)?;
    }

    // extract an atomic pipeline from back of the pipeline and stash preceding part into context
    let pipeline = anchor::extract_atomic(pipeline, &mut ctx.anchor);

    // ensure names for all columns that need it
    ensure_names(&pipeline, &mut ctx.anchor);

    log::trace!("compiling pipeline {pipeline:#?}");

    let mut c = TransformCompiler { ctx };
    let pipeline = c.fold_sql_transforms(pipeline)?;

    Ok(SqlRelation::AtomicPipeline(pipeline))
}

struct TransformCompiler<'a> {
    ctx: &'a mut Context,
}

impl<'a> RqFold for TransformCompiler<'a> {}

impl<'a> SrqFold<TableRef, RelationExpr, Transform, ()> for TransformCompiler<'a> {
    fn fold_rel(&mut self, rel: TableRef) -> Result<RelationExpr> {
        compile_table_ref(rel, self.ctx)
    }

    fn fold_super(&mut self, _: Transform) -> Result<()> {
        unreachable!()
    }

    fn fold_sql_transforms(
        &mut self,
        transforms: Vec<SqlTransform<TableRef, Transform>>,
    ) -> Result<Vec<SqlTransform<RelationExpr, ()>>> {
        transforms
            .into_iter()
            .map(|transform| {
                Ok(Some(if let SqlTransform::Super(sup) = transform {
                    match sup {
                        Transform::From(v) => SqlTransform::From(self.fold_rel(v)?),

                        Transform::Select(v) => SqlTransform::Select(v),
                        Transform::Filter(v) => SqlTransform::Filter(v),
                        Transform::Aggregate { partition, compute } => {
                            SqlTransform::Aggregate { partition, compute }
                        }
                        Transform::Sort(v) => SqlTransform::Sort(v),
                        Transform::Take(v) => SqlTransform::Take(v),
                        Transform::Join { side, with, filter } => SqlTransform::Join {
                            side,
                            with: self.fold_rel(with)?,
                        filter,
                        },
                        Transform::Compute(_) | Transform::Append(_) | Transform::Loop(_) => {
                            return Ok(None)
                        }
                    }
                } else {
                    fold_sql_transform(self, transform)?
                }))
            })
            .flat_map(|x| x.transpose())
            .try_collect()
    }
}

pub(super) fn compile_table_ref(table_ref: TableRef, ctx: &mut Context) -> Result<RelationExpr> {
    let decl = ctx.anchor.table_decls.get_mut(&table_ref.source).unwrap();

    // ensure that the table is declared
    if let RelationStatus::NotYetDefined(sql_relation) = decl.relation.take_to_define() {
        // if we cannot use CTEs (probably because we are within RECURSIVE)
        if !ctx.query.allow_ctes {
            // restore relation for other references
            decl.relation = RelationStatus::NotYetDefined(sql_relation.clone().into());

            // return a sub-query
            let relation = compile_relation(sql_relation, ctx)?;
            return Ok(RelationExpr::SubQuery(relation, table_ref.name));
        }

        let relation = compile_relation(sql_relation, ctx)?;
        ctx.ctes.push((table_ref.source, relation));
    }

    Ok(RelationExpr::Ref(table_ref.source, table_ref.name))
}

#[allow(dead_code, unused_variables)]
fn sql_of_loop(pipeline: Vec<SqlTransform>, ctx: &mut Context) -> Result<Vec<SqlTransform>> {
    /*
    // split the pipeline
    let (mut initial, mut following) = pipeline.break_up(|t| matches!(t, SqlTransform::Loop(_)));
    let loop_ = following.remove(0);
    let step = loop_.into_loop().unwrap();

    // RECURSIVE can only follow WITH directly, which means that if we want to use it for
    // an arbitrary query, we have to defined a *nested* WITH RECURSIVE and not use
    // the top-level list of CTEs.

    // determine columns of the initial table
    let recursive_columns = AnchorContext::determine_select_columns(&initial);

    // do the same thing we do when splitting a pipeline
    // (defining new columns, redirecting cids)
    let recursive_columns = SqlTransform::Super(Transform::Select(recursive_columns));
    initial.push(recursive_columns.clone());
    let (step, _) = anchor_split(&mut ctx.anchor, initial, step);
    let from = step.first().unwrap().as_super().unwrap().as_from().unwrap();

    let table_name = "_loop";
    let initial = ctx.anchor.table_decls.get_mut(&from.source).unwrap();
    initial.name = Some(table_name.to_string());
    let initial_relation = if let RelationStatus::NotYetDefined(rel) = initial.relation {
        compile_relation(rel, ctx)?
    } else {
        unreachable!()
    };

    let (initial, _) = initial_relation.into_pipeline().unwrap();

    // compile initial
    let initial = query_to_set_expr(sql_query_of_pipeline(initial, ctx)?, ctx);

    // compile step (without producing CTEs)
    ctx.push_query();
    ctx.query.allow_ctes = false;

    let step = query_to_set_expr(sql_query_of_pipeline(step, ctx)?, ctx);

    ctx.pop_query();

    // build CTE and it's SELECT
    let cte = sql_ast::Cte {
        alias: simple_table_alias(Ident::new(table_name)),
        query: Box::new(default_query(SetExpr::SetOperation {
            op: sql_ast::SetOperator::Union,
            set_quantifier: sql_ast::SetQuantifier::All,
            left: initial,
            right: step,
        })),
        from: None,
    };
    let query = Box::new(sql_ast::Query {
        with: Some(sql_ast::With {
            recursive: true,
            cte_tables: vec![cte],
        }),
        ..default_query(sql_ast::SetExpr::Select(Box::new(sql_ast::Select {
            projection: vec![SelectItem::Wildcard(
                sql_ast::WildcardAdditionalOptions::default(),
            )],
            from: vec![TableWithJoins {
                relation: TableFactor::Table {
                    name: sql_ast::ObjectName(vec![Ident::new(table_name)]),
                    alias: None,
                    args: None,
                    with_hints: Vec::new(),
                },
                joins: vec![],
            }],
            ..default_select()
        })))
    });

    // create a split between the loop SELECT statement and the following pipeline
    let (mut following, _) = anchor_split(&mut ctx.anchor, vec![recursive_columns], following);

    let from = following.first_mut().unwrap();
    let from = from.as_super().unwrap().as_from().unwrap();

    // this will be table decl that references the whole loop expression
    let loop_decl = ctx.anchor.table_decls.get_mut(&from.source).unwrap();

    let loop_name = ctx.anchor.table_name.gen();
    loop_decl.name = Some(loop_name.clone());
    loop_decl.relation = RelationStatus::Defined;

    // push the whole thing into WITH of the main query
    ctx.ctes.push((loop_name, query));

    Ok(following)
     */
    todo!()
}

fn ensure_names(transforms: &[SqlTransform<TableRef>], ctx: &mut AnchorContext) {
    let empty = HashSet::new();
    for t in transforms {
        match t {
            SqlTransform::Super(Transform::Sort(_)) => {
                for r in anchor::get_requirements(t, &empty) {
                    ctx.ensure_column_name(r.col);
                }
            }
            SqlTransform::Super(Transform::Select(cids)) => {
                for cid in cids {
                    let _decl = &ctx.column_decls[cid];
                    //let name = match decl {
                    //    ColumnDecl::RelationColumn(_, _, _) => todo!(),
                    //    ColumnDecl::Compute(_) => ctx.column_names[..],
                    //};
                }
            }
            _ => (),
        }
    }
}
