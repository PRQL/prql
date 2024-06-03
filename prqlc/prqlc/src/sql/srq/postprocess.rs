//! An AST pass after compilation to SRQ.
//!
//! Currently only moves [SqlTransform::Sort]s.

use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use super::anchor::CidRedirector;
use super::ast::*;
use crate::ir::generic::ColumnSort;
use crate::ir::pl::Ident;
use crate::ir::rq::{CId, RqFold, TId};
use crate::sql::Context;
use crate::Result;

type Sorting = Vec<ColumnSort<CId>>;

pub(super) fn postprocess(query: SqlQuery, ctx: &mut Context) -> SqlQuery {
    let query = infer_sorts(query, ctx);

    assign_names(query, ctx)
}

/// Pushes sorts down the pipelines and materializes them only where they are needed.
fn infer_sorts(query: SqlQuery, ctx: &mut Context) -> SqlQuery {
    let mut s = SortingInference {
        last_sorting: Vec::new(),
        ctes_sorting: HashMap::new(),
        ctx,
    };

    s.fold_sql_query(query).unwrap()
}

struct SortingInference<'a> {
    last_sorting: Sorting,
    ctes_sorting: HashMap<TId, CteSorting>,
    ctx: &'a mut Context,
}

struct CteSorting {
    sorting: Sorting,
    has_been_used: bool,
}

impl<'a> RqFold for SortingInference<'a> {}

impl<'a> SrqFold for SortingInference<'a> {
    fn fold_sql_query(&mut self, query: SqlQuery) -> Result<SqlQuery> {
        let mut ctes = Vec::with_capacity(query.ctes.len());
        for cte in query.ctes {
            let cte = self.fold_cte(cte)?;

            // store sorting to be used later in From references
            let sorting = self.last_sorting.drain(..).collect();
            let sorting = CteSorting {
                sorting,
                has_been_used: false,
            };
            self.ctes_sorting.insert(cte.tid, sorting);

            ctes.push(cte);
        }

        // fold main_relation using a made-up tid
        let mut main_relation = self.fold_sql_relation(query.main_relation)?;

        // push a sort at the back of the main pipeline
        if let SqlRelation::AtomicPipeline(pipeline) = &mut main_relation {
            pipeline.push(SqlTransform::Sort(self.last_sorting.drain(..).collect()));
        }

        // make sure that all CTEs whose sorting was used actually SELECT it
        for cte in &mut ctes {
            let sorting = self.ctes_sorting.get(&cte.tid).unwrap();
            if !sorting.has_been_used {
                continue;
            }

            let CteKind::Normal(sql_relation) = &mut cte.kind else {
                continue;
            };
            let Some(pipeline) = sql_relation.as_atomic_pipeline_mut() else {
                continue;
            };
            let select = pipeline.iter_mut().find_map(|x| x.as_select_mut()).unwrap();

            for column_sort in &sorting.sorting {
                let cid = column_sort.column;
                let is_selected = select.contains(&cid);
                if !is_selected {
                    select.push(cid);
                }
            }
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
                            if let Some(cte_sorting) = self.ctes_sorting.get_mut(tid) {
                                cte_sorting.has_been_used = true;
                                sorting = cte_sorting.sorting.clone();
                            } else {
                                sorting = Vec::new();
                            };
                        }
                        RelationExprKind::SubQuery(rel) => {
                            let rel = self.fold_sql_relation(rel)?;

                            // infer sorting from sub-query
                            sorting = self.last_sorting.drain(..).collect();

                            expr.kind = RelationExprKind::SubQuery(rel);
                        }
                    }
                    sorting =
                        CidRedirector::redirect_sorts(sorting, &expr.riid, &mut self.ctx.anchor);
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
                SqlTransform::Take(_) | SqlTransform::DistinctOn(_) => {
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

/// Makes sure all relation instances have assigned names. Tries to infer from table references.
fn assign_names(query: SqlQuery, ctx: &mut Context) -> SqlQuery {
    // generate CTE names, make sure they don't clash
    let decls = ctx.anchor.table_decls.values_mut();
    let mut names = HashSet::new();
    for decl in decls.sorted_by_key(|d| d.id.get()) {
        while decl.name.is_none() || names.contains(decl.name.as_ref().unwrap()) {
            decl.name = Some(Ident::from_name(ctx.anchor.table_name.gen()));
        }
        names.insert(decl.name.clone().unwrap());
    }

    // generate relation variable names
    RelVarNameAssigner {
        ctx,
        relation_instance_names: Default::default(),
    }
    .fold_sql_query(query)
    .unwrap()
}

struct RelVarNameAssigner<'a> {
    relation_instance_names: HashSet<String>,

    ctx: &'a mut Context,
}

impl<'a> RqFold for RelVarNameAssigner<'a> {}

impl<'a> SrqFold for RelVarNameAssigner<'a> {
    fn fold_sql_relation(&mut self, relation: SqlRelation) -> Result<SqlRelation> {
        // only fold AtomicPipelines
        Ok(match relation {
            SqlRelation::AtomicPipeline(pipeline) => {
                // save outer names, so they are not affected by the inner pipeline
                // (this matters for loop, where you have nested pipelines)
                let outer_names = std::mem::take(&mut self.relation_instance_names);

                let res = self.fold_sql_transforms(pipeline)?;

                self.relation_instance_names = outer_names;
                SqlRelation::AtomicPipeline(res)
            }
            _ => relation,
        })
    }
}

impl<'a> SrqMapper<RelationExpr, RelationExpr, (), ()> for RelVarNameAssigner<'a> {
    fn fold_rel(&mut self, mut rel: RelationExpr) -> Result<RelationExpr> {
        // normal fold
        rel.kind = match rel.kind {
            RelationExprKind::Ref(tid) => RelationExprKind::Ref(tid),
            RelationExprKind::SubQuery(sub) => {
                RelationExprKind::SubQuery(self.fold_sql_relation(sub)?)
            }
        };

        // make sure that table_ref has a name
        let riid = &rel.riid;
        let instance = self.ctx.anchor.relation_instances.get_mut(riid).unwrap();
        let name = &mut instance.table_ref.name;

        if name.is_none() {
            // it does not

            // infer from table name
            *name = match &rel.kind {
                RelationExprKind::Ref(tid) => {
                    let table_decl = &self.ctx.anchor.table_decls[tid];
                    table_decl.name.as_ref().map(|i| i.name.clone())
                }
                _ => None,
            };
        }

        // make sure it is not already present in current query
        while name
            .as_ref()
            .map_or(true, |n| self.relation_instance_names.contains(n))
        {
            *name = Some(self.ctx.anchor.table_name.gen());
        }

        // mark name as used
        self.relation_instance_names.insert(name.clone().unwrap());

        Ok(rel)
    }

    fn fold_super(&mut self, sup: ()) -> Result<()> {
        Ok(sup)
    }
}
