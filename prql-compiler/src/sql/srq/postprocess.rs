use std::collections::HashMap;

use anyhow::Result;

use crate::ast::pl::ColumnSort;
use crate::ast::rq::{CId, RqFold, TId};
use crate::sql::Context;

use super::anchor::CidRedirector;
use super::ast::*;

type Sorting = Vec<ColumnSort<CId>>;

pub(super) fn postprocess(query: SqlQuery, ctx: &mut Context) -> Result<SqlQuery> {
    let mut s = SortingInference {
        last_sorting: Vec::new(),
        sortings: HashMap::new(),
        ctx,
    };

    s.fold_sql_query(query)
}

struct SortingInference<'a> {
    last_sorting: Sorting,
    sortings: HashMap<TId, Sorting>,
    ctx: &'a mut Context,
}

impl<'a> RqFold for SortingInference<'a> {}

impl<'a> SrqFold for SortingInference<'a> {
    fn fold_sql_query(&mut self, query: SqlQuery) -> Result<SqlQuery> {
        let mut ctes = Vec::with_capacity(query.ctes.len());
        for cte in query.ctes {
            let cte = self.fold_cte(cte)?;

            // store sorting to be used later in From references
            let sorting = self.last_sorting.drain(..).collect();
            self.sortings.insert(cte.tid, sorting);

            ctes.push(cte);
        }

        // fold main_relation using a made-up tid
        let mut main_relation = self.fold_sql_relation(query.main_relation)?;

        // push a sort at the back of the main pipeline
        if let SqlRelation::AtomicPipeline(pipeline) = &mut main_relation {
            pipeline.push(SqlTransform::Sort(self.last_sorting.drain(..).collect()))
        }

        Ok(SqlQuery {
            ctes,
            main_relation,
        })
    }
}

impl<'a> SrqMapper<RelationExpr, RelationExpr, (), ()> for SortingInference<'a> {
    fn fold_rel(&mut self, rel: RelationExpr) -> Result<RelationExpr> {
        Ok(rel)
    }

    fn fold_super(&mut self, sup: ()) -> Result<()> {
        Ok(sup)
    }

    fn fold_sql_transforms(
        &mut self,
        transforms: Vec<SqlTransform<RelationExpr, ()>>,
    ) -> Result<Vec<SqlTransform<RelationExpr, ()>>> {
        let mut sorting = Vec::new();

        let mut result = Vec::with_capacity(transforms.len() + 1);

        for mut transform in transforms {
            match transform {
                SqlTransform::From(mut expr) => {
                    match expr.kind {
                        RelationExprKind::Ref(ref tid) => {
                            // infer sorting from referenced pipeline
                            sorting = self.sortings.get(tid).cloned().unwrap_or_default();
                        }
                        RelationExprKind::SubQuery(rel) => {
                            let rel = self.fold_sql_relation(rel)?;

                            // infer sorting from sub-query
                            sorting = self.last_sorting.drain(..).collect();

                            expr.kind = RelationExprKind::SubQuery(rel);
                        }
                    }
                    if let Some(riid) = &expr.riid {
                        sorting = CidRedirector::redirect_sorts(sorting, riid, &mut self.ctx.anchor)
                    }
                    transform = SqlTransform::From(expr);
                }

                // just store sorting and don't emit Sort
                SqlTransform::Sort(s) => {
                    sorting = s.clone();
                    continue;
                }

                // clear sorting
                SqlTransform::Distinct | SqlTransform::Aggregate { .. } => {
                    sorting = Vec::new();
                }

                // emit Sort before Take
                SqlTransform::Take(_) => {
                    result.push(SqlTransform::Sort(sorting.clone()));
                }
                _ => {}
            }
            result.push(transform)
        }

        // remember sorting for this pipeline
        self.last_sorting = sorting;

        Ok(result)
    }
}
