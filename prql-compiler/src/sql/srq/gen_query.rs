//! This module is responsible for translating RQ to SRQ.

use std::collections::HashSet;
use std::str::FromStr;

use anyhow::Result;
use itertools::Itertools;

use crate::ast::pl::Ident;
use crate::ast::rq::{Query, RelationKind, RqFold, TableRef, Transform};
use crate::utils::BreakUp;
use crate::Target;

use super::anchor::{self, anchor_split};
use super::ast::{
    fold_sql_transform, Cte, CteKind, RelationExpr, RelationExprKind, SqlQuery, SqlRelation,
    SqlTransform, SrqMapper,
};
use super::context::{AnchorContext, RelationAdapter, RelationStatus};

use super::super::{Context, Dialect};
use super::{postprocess, preprocess};

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

    let query = postprocess::postprocess(query, &mut ctx)?;

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
                RelationKind::BuiltInFunction { name, args } => {
                    SqlRelation::Operator { name, args }
                }

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
    mut pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<SqlRelation> {
    use SqlTransform::Super;

    // special case: loop
    if pipeline
        .iter()
        .any(|t| matches!(t, Super(Transform::Loop(_))))
    {
        pipeline = compile_loop(pipeline, ctx)?;
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

impl<'a> SrqMapper<TableRef, RelationExpr, Transform, ()> for TransformCompiler<'a> {
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
                            // these are not used from here on
                            return Ok(None);
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
    let relation_instance = ctx.anchor.find_relation_instance(&table_ref);
    let riid = relation_instance.map(|r| r.riid);
    let alias = table_ref.name;

    let decl = ctx.anchor.table_decls.get_mut(&table_ref.source).unwrap();

    // ensure that the table is declared
    if let RelationStatus::NotYetDefined(sql_relation) = decl.relation.take_to_define() {
        // if we cannot use CTEs (probably because we are within RECURSIVE)
        if !ctx.query.allow_ctes {
            // restore relation for other references
            decl.relation = RelationStatus::NotYetDefined(sql_relation.clone());

            // return a sub-query
            let relation = compile_relation(sql_relation, ctx)?;
            return Ok(RelationExpr {
                kind: RelationExprKind::SubQuery(relation),
                alias,
                riid,
            });
        }

        let relation = compile_relation(sql_relation, ctx)?;
        ctx.ctes.push(Cte {
            tid: table_ref.source,
            kind: CteKind::Normal(relation),
        });
    }

    Ok(RelationExpr {
        kind: RelationExprKind::Ref(table_ref.source),
        alias,
        riid,
    })
}

fn compile_loop(
    pipeline: Vec<SqlTransform<TableRef>>,
    ctx: &mut Context,
) -> Result<Vec<SqlTransform<TableRef>>> {
    // split the pipeline
    let (mut initial, mut following) =
        pipeline.break_up(|t| matches!(t, SqlTransform::Super(Transform::Loop(_))));
    let loop_ = following.remove(0);
    let step = loop_.into_super_and(|t| t.into_loop()).unwrap();
    let step = preprocess::preprocess(step, ctx)?;

    // determine columns of the initial table
    let recursive_columns = AnchorContext::determine_select_columns(&initial);

    // do the same thing we do when splitting a pipeline
    // (defining new columns, redirecting cids)
    let recursive_columns = SqlTransform::Super(Transform::Select(recursive_columns));
    initial.push(recursive_columns.clone());
    let step = anchor_split(&mut ctx.anchor, initial, step);
    let from = step.first().unwrap().as_super().unwrap().as_from().unwrap();

    let recursive_name = ctx.anchor.table_name.gen();
    let initial = ctx.anchor.table_decls.get_mut(&from.source).unwrap();
    initial.name = Some(Ident::from_name(recursive_name.clone()));

    // compile initial
    let initial = if let RelationStatus::NotYetDefined(rel) = initial.relation.take_to_define() {
        compile_relation(rel, ctx)?
    } else {
        unreachable!()
    };

    // compile step (without producing CTEs)
    ctx.push_query();
    ctx.query.allow_ctes = false;

    let step = compile_pipeline(step, ctx)?;

    ctx.pop_query();

    // create a split between the loop SELECT statement and the following pipeline
    let mut following = anchor_split(&mut ctx.anchor, vec![recursive_columns], following);

    let from = following.first_mut().unwrap();
    let from = from.as_super().unwrap().as_from().unwrap();

    // this will be table decl that references the whole loop expression
    let loop_decl = ctx.anchor.table_decls.get_mut(&from.source).unwrap();

    loop_decl.name = Some(Ident::from_name(recursive_name));
    loop_decl.relation = RelationStatus::Defined;

    // push the whole thing into WITH of the main query
    ctx.ctes.push(Cte {
        tid: from.source,
        kind: CteKind::Loop { initial, step },
    });

    Ok(following)
}

fn ensure_names(transforms: &[SqlTransform<TableRef>], ctx: &mut AnchorContext) {
    let empty = HashSet::new();
    for t in transforms {
        if let SqlTransform::Super(Transform::Sort(_)) = t {
            for r in anchor::get_requirements(t, &empty) {
                ctx.ensure_column_name(r.col);
            }
        }
    }
}
