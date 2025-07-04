//! This module is responsible for translating RQ to PQ.

use std::str::FromStr;

use itertools::Itertools;

use super::super::{Context, Dialect};
use super::anchor::{self, anchor_split};
use super::ast::{self as pq, fold_sql_transform, PqMapper};
use super::context::{AnchorContext, RIId, RelationAdapter, RelationStatus};
use super::{postprocess, preprocess};
use crate::debug;
use crate::ir::rq::{self, RqFold};
use crate::utils::BreakUp;
use crate::{Result, Target};

pub(in super::super) fn compile_query(
    query: rq::RelationalQuery,
    dialect: Option<Dialect>,
) -> Result<(pq::SqlQuery, Context)> {
    debug::log_stage(debug::Stage::Sql(debug::StageSql::Anchor));

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

    let query = pq::SqlQuery {
        main_relation,
        ctes,
    };
    debug::log_entry(|| debug::DebugEntryKind::ReprPq(query.clone()));

    debug::log_stage(debug::Stage::Sql(debug::StageSql::Postprocess));
    let query = postprocess::postprocess(query, &mut ctx);
    debug::log_entry(|| debug::DebugEntryKind::ReprPq(query.clone()));

    Ok((query, ctx))
}

fn compile_relation(relation: RelationAdapter, ctx: &mut Context) -> Result<pq::SqlRelation> {
    log::trace!("compiling relation {relation:#?}");

    Ok(match relation {
        RelationAdapter::Rq(rel) => {
            match rel.kind {
                // base case
                rq::RelationKind::Pipeline(pipeline) => {
                    // preprocess
                    let pipeline = preprocess::preprocess(pipeline, ctx)?;

                    // load names of output columns
                    ctx.anchor.load_names(&pipeline, rel.columns);

                    compile_pipeline(pipeline, ctx)?
                }

                rq::RelationKind::Literal(lit) => pq::SqlRelation::Literal(lit),
                rq::RelationKind::SString(items) => pq::SqlRelation::SString(items),
                rq::RelationKind::BuiltInFunction { name, args } => {
                    pq::SqlRelation::Operator { name, args }
                }

                // ref cannot be converted directly into query and does not need it's own CTE
                rq::RelationKind::ExternRef(_) => unreachable!(),
            }
        }

        RelationAdapter::Preprocessed(pipeline, columns) => {
            // load names of output columns
            ctx.anchor.load_names(&pipeline, columns);

            compile_pipeline(pipeline, ctx)?
        }

        RelationAdapter::Pq(rel) => rel,
    })
}

fn compile_pipeline(
    mut pipeline: Vec<pq::SqlTransform>,
    ctx: &mut Context,
) -> Result<pq::SqlRelation> {
    use pq::SqlTransform::Super;

    // special case: loop
    if pipeline
        .iter()
        .any(|t| matches!(t, Super(rq::Transform::Loop(_))))
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

    Ok(pq::SqlRelation::AtomicPipeline(pipeline))
}

struct TransformCompiler<'a> {
    ctx: &'a mut Context,
}

impl RqFold for TransformCompiler<'_> {}

impl PqMapper<RIId, pq::RelationExpr, rq::Transform, ()> for TransformCompiler<'_> {
    fn fold_rel(&mut self, rel: RIId) -> Result<pq::RelationExpr> {
        compile_relation_instance(rel, self.ctx)
    }

    fn fold_super(&mut self, _: rq::Transform) -> Result<()> {
        unreachable!()
    }

    fn fold_sql_transforms(
        &mut self,
        transforms: Vec<pq::SqlTransform<RIId, rq::Transform>>,
    ) -> Result<Vec<pq::SqlTransform<pq::RelationExpr, ()>>> {
        transforms
            .into_iter()
            .map(|transform| {
                Ok(Some(match transform {
                    pq::SqlTransform::From(v) => pq::SqlTransform::From(self.fold_rel(v)?),
                    pq::SqlTransform::Join { side, with, filter } => pq::SqlTransform::Join {
                        side,
                        with: self.fold_rel(with)?,
                        filter,
                    },

                    pq::SqlTransform::Super(sup) => {
                        match sup {
                            rq::Transform::Select(v) => pq::SqlTransform::Select(v),
                            rq::Transform::Filter(v) => pq::SqlTransform::Filter(v),
                            rq::Transform::Aggregate { partition, compute } => {
                                pq::SqlTransform::Aggregate { partition, compute }
                            }
                            rq::Transform::Sort(v) => pq::SqlTransform::Sort(v),
                            rq::Transform::Take(v) => pq::SqlTransform::Take(v),
                            rq::Transform::Compute(_)
                            | rq::Transform::Append(_)
                            | rq::Transform::Loop(_) => {
                                // these are not used from here on
                                return Ok(None);
                            }
                            rq::Transform::From(_) | rq::Transform::Join { .. } => unreachable!(),
                        }
                    }
                    _ => fold_sql_transform(self, transform)?,
                }))
            })
            .flat_map(|x| x.transpose())
            .try_collect()
    }
}

pub(super) fn compile_relation_instance(riid: RIId, ctx: &mut Context) -> Result<pq::RelationExpr> {
    ctx.anchor.positional_mapping.activate_mapping(&riid);

    let rel_instance = &ctx.anchor.relation_instances[&riid];
    let nb_redirects = rel_instance.cid_redirects.len();
    let table_ref = &rel_instance.table_ref;
    let source = table_ref.source;
    let decl = ctx.anchor.table_decls.get_mut(&table_ref.source).unwrap();

    // ensure that the table is declared
    if let RelationStatus::NotYetDefined(sql_relation) = decl.relation.take_to_define() {
        // if we cannot use CTEs (probably because we are within RECURSIVE)
        if !(ctx.query.allow_ctes && table_ref.prefer_cte) {
            // restore relation for other references
            decl.relation = RelationStatus::NotYetDefined(sql_relation.clone());

            // return a sub-query
            let relation = compile_relation(sql_relation, ctx)?;
            return Ok(pq::RelationExpr {
                kind: pq::RelationExprKind::SubQuery(relation),
                riid,
            });
        }

        let relation = compile_relation(sql_relation, ctx)?;

        if let pq::SqlRelation::AtomicPipeline(pipeline) = &relation {
            // Finding the last select statement of the pipeline
            let last_select_columns = pipeline.iter().rev().find_map(|transform| match transform {
                pq::SqlTransform::Select(cids) => Some(cids),
                _ => None,
            });

            log::debug!("last select CIds for {riid:?}: {last_select_columns:?}");

            // If the pipeline ends with a select, we must recompute its CId redirects
            if let Some(cids) = last_select_columns {
                // Only recompute the CId redirects if there are exactly as many columns in the
                // SELECT as there are CId redirects. This probably means that it is a projecting
                // select added by `anchor_split`
                if nb_redirects == cids.len() {
                    log::debug!(
                        "recomputing cid_redirects for {riid:?}. current redirects: {:?}",
                        ctx.anchor.relation_instances[&riid].cid_redirects
                    );
                    // Inefficient but only way to ensure that the new redirects match the original cids
                    let new_redirects = cids
                        .iter()
                        .zip(&ctx.anchor.relation_instances[&riid].original_cids)
                        .map(|(new_cid, original_cid)| {
                            let key_for_value = ctx.anchor.relation_instances[&riid]
                                .cid_redirects
                                .iter()
                                .find_map(|(k, v)| if v == original_cid { Some(k) } else { None })
                                .unwrap();

                            (
                                *new_cid,
                                ctx.anchor.relation_instances[&riid].cid_redirects[key_for_value],
                            )
                        })
                        .collect();

                    log::debug!(
                        "recomputed cid_redirects for {riid:?}. new redirects: {new_redirects:?}",
                    );

                    ctx.anchor
                        .relation_instances
                        .get_mut(&riid)
                        .unwrap()
                        .cid_redirects = new_redirects;
                }
            }
        }
        ctx.ctes.push(pq::Cte {
            tid: source,
            kind: pq::CteKind::Normal(relation),
        });
    }

    Ok(pq::RelationExpr {
        kind: pq::RelationExprKind::Ref(source),
        riid,
    })
}

fn compile_loop(
    pipeline: Vec<pq::SqlTransform>,
    ctx: &mut Context,
) -> Result<Vec<pq::SqlTransform>> {
    // split the pipeline
    let (mut initial, mut following) =
        pipeline.break_up(|t| matches!(t, pq::SqlTransform::Super(rq::Transform::Loop(_))));
    let loop_ = following.remove(0);
    let step = loop_.into_super_and(|t| t.into_loop()).unwrap();
    let step = preprocess::preprocess(step, ctx)?;

    // determine columns of the initial table
    let recursive_columns = ctx.anchor.determine_select_columns(&initial);

    // do the same thing we do when splitting a pipeline
    // (defining new columns, redirecting cids)
    let recursive_columns = pq::SqlTransform::Super(rq::Transform::Select(recursive_columns));
    initial.push(recursive_columns.clone());
    let step = anchor_split(&mut ctx.anchor, initial, step);
    let from = step.first().unwrap().as_from().unwrap();
    let from = ctx.anchor.relation_instances.get(from).unwrap();
    let initial_tid = from.table_ref.source;

    let initial = ctx.anchor.table_decls.get_mut(&initial_tid).unwrap();

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
    let from = from.as_from().unwrap();
    let from = ctx.anchor.relation_instances.get(from).unwrap();
    let from = &from.table_ref;

    // this will be table decl that references the whole loop expression
    let loop_decl = ctx.anchor.table_decls.get_mut(&from.source).unwrap();

    loop_decl.redirect_to = Some(initial_tid);
    loop_decl.relation = RelationStatus::Defined;

    // push the whole thing into WITH of the main query
    ctx.ctes.push(pq::Cte {
        tid: from.source,
        kind: pq::CteKind::Loop { initial, step },
    });

    Ok(following)
}

fn ensure_names(transforms: &[pq::SqlTransform], ctx: &mut AnchorContext) {
    for t in transforms {
        if let pq::SqlTransform::Super(rq::Transform::Sort(columns))
        | pq::SqlTransform::Sort(columns) = t
        {
            for r in columns {
                ctx.ensure_column_name(r.column);
            }
        }
    }
}
