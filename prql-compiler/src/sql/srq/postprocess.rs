use std::collections::HashMap;

use anyhow::Result;

use crate::ast::pl::ColumnSort;
use crate::ast::rq::{CId, RqFold, TId};

use super::ast::*;

type Sorting = Vec<ColumnSort<CId>>;

pub fn postprocess(query: SqlQuery) -> Result<SqlQuery> {
    let mut s = SortingInference::default();

    s.fold_sql_query(query)
}

#[derive(Default)]
struct SortingInference {
    last_sorting: Sorting,
    sortings: HashMap<TId, Sorting>,
}

impl RqFold for SortingInference {}

impl SrqFold for SortingInference {
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

impl SrqMapper<RelationExpr, RelationExpr, (), ()> for SortingInference {
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
                    match expr {
                        RelationExpr::Ref(ref tid, _) => {
                            // infer sorting from referenced pipeline
                            sorting = self.sortings.get(tid).cloned().unwrap_or_default();
                        }
                        RelationExpr::SubQuery(rel, alias) => {
                            let rel = self.fold_sql_relation(rel)?;

                            // infer sorting from sub-query
                            sorting = self.last_sorting.drain(..).collect();

                            expr = RelationExpr::SubQuery(rel, alias);
                        }
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
