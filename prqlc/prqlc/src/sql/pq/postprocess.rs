//! An AST pass after compilation to PQ.
//!
//! Currently only moves [SqlTransform::Sort]s.

use std::collections::{HashMap, HashSet, VecDeque};

use itertools::Itertools;

use super::anchor::CidRedirector;
use super::ast::*;
use crate::ir::generic::ColumnSort;
use crate::ir::pl::Ident;
use crate::ir::rq::{CId, ExprKind, RqFold, TId};
use crate::sql::pq::context::{ColumnDecl, RIId};
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
        main_relation: false,
        ctx,
    };

    s.fold_sql_query(query).unwrap()
}

struct SortingInference<'a> {
    last_sorting: Sorting,
    ctes_sorting: HashMap<TId, CteSorting>,
    main_relation: bool,
    ctx: &'a mut Context,
}

impl SortingInference<'_> {
    /// Prepares the last sorting that will be appended to the pipeline of the `SqlQuery` by
    /// `fold_sql_query`. It does so by reverting all columns in the sorting to their very first
    /// form, and then transforming their value in the final select, while applying
    /// renaming/aliasing when possible. This cannot be done directly in `fold_sql_transforms`
    /// because renames are not considered to be SQL transforms.
    fn alias_last_sorting(&mut self, mut last_sorting: Sorting, final_select: &[CId]) -> Sorting {
        log::debug!("unaliasing last sorting: {last_sorting:?}");
        let redirects = self
            .ctx
            .anchor
            .relation_instances
            .iter()
            .map(|(riid, rel_inst)| (riid, &rel_inst.cid_redirects))
            .collect::<HashMap<_, _>>();

        // a map of column -> alias
        let column_aliases = self
            .ctx
            .anchor
            .column_decls
            .values()
            .filter_map(|col| {
                if let ColumnDecl::Compute(compute) = col {
                    if let ExprKind::ColumnRef(referenced_id) = compute.expr.kind {
                        Some((referenced_id, compute.id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();
        log::debug!(".. column aliases: {column_aliases:?}");

        // column -> list of tables that did a revert
        let mut reverts: HashMap<CId, VecDeque<RIId>> = HashMap::new();
        log::debug!(".. reverting all columns to their original value");
        last_sorting.iter_mut().for_each(|sort| {
            let mut riids = VecDeque::new();
            let mut changed = true;
            while changed {
                changed = false;
                if let Some(ColumnDecl::RelationColumn(riid, cid, _)) =
                    self.ctx.anchor.column_decls.get(&sort.column)
                {
                    let cid_redirects = redirects[riid];
                    for (source, target) in cid_redirects.iter() {
                        if target == cid {
                            log::debug!(
                                ".. reverting {target:?} back to {source:?} via redirects of {riid:?}"
                            );
                            sort.column = *source;
                            changed = true;
                            riids.push_front(*riid);
                            break;
                        }
                    }
                }
            }
            reverts.insert(sort.column, riids);
        });
        log::debug!(".. done reverting all columns to their original value: {last_sorting:?}");

        log::debug!(".. reverting columns forward and aliasing them");
        // reverting forward
        last_sorting.iter_mut().for_each(|sort| {
            let col_reverts = &reverts[&sort.column];
            for riid in col_reverts {
                if final_select.contains(&sort.column) {
                    log::debug!(
                        ".. sort column {:?} is in the final select columns, skip reverting",
                        &sort.column
                    );
                    return;
                }
                // try renaming
                if column_aliases.contains_key(&sort.column) {
                    let alias = column_aliases[&sort.column];
                    log::debug!("..aliasing {:?} as {alias:?}", &sort.column);
                    sort.column = alias;
                }
                // try de-reverting with the target table
                let cid_mappings = redirects[riid];
                if cid_mappings.contains_key(&sort.column) {
                    log::debug!(
                        ".. reverting {:?} forward to {:?} via redirects of {riid:?} ({:?})",
                        &sort.column,
                        &cid_mappings[&sort.column],
                        &cid_mappings
                    );
                    sort.column = cid_mappings[&sort.column];
                }
            }
        });

        log::debug!("aliased and reverted last sorting forward: {last_sorting:?}");

        last_sorting
    }
}

#[derive(Debug)]
struct CteSorting {
    sorting: Sorting,
}

impl RqFold for SortingInference<'_> {}

impl PqFold for SortingInference<'_> {
    fn fold_sql_query(&mut self, query: SqlQuery) -> Result<SqlQuery> {
        let mut ctes = Vec::with_capacity(query.ctes.len());

        for cte in query.ctes {
            log::debug!("infer_sorts: {0:?}", cte.tid);
            let cte = self.fold_cte(cte)?;

            // store sorting to be used later in From references
            let sorting = self.last_sorting.drain(..).collect();
            log::debug!("--- sorting {sorting:?}");
            let sorting = CteSorting { sorting };
            self.ctes_sorting.insert(cte.tid, sorting);

            ctes.push(cte);
        }

        // fold main_relation
        log::debug!("infer_sorts: main relation");
        self.main_relation = true;
        let mut main_relation = self.fold_sql_relation(query.main_relation)?;
        log::debug!("--== last_sorting {0:?}", self.last_sorting);
        let last_sorting = self.last_sorting.drain(..).collect::<Vec<_>>();

        // push a sort at the back of the main pipeline
        if let SqlRelation::AtomicPipeline(pipeline) = &mut main_relation {
            let from_id = pipeline
                .iter()
                .find_map(|transform| match transform {
                    SqlTransform::From(rel) => Some(rel.riid),
                    _ => None,
                })
                .unwrap();

            let final_select = pipeline
                .iter()
                .rev()
                .find_map(|transform| match transform {
                    SqlTransform::Select(select) => Some(select),
                    _ => None,
                })
                .unwrap();
            log::debug!("--== final select: {final_select:?}");

            let unaliased_last_sorting = self.alias_last_sorting(last_sorting, final_select);
            log::debug!("--== unaliased last sorting: {unaliased_last_sorting:?}");
            let redirected_last_sorting = CidRedirector::redirect_sorts(
                unaliased_last_sorting,
                &from_id,
                &mut self.ctx.anchor,
            );
            log::debug!("--== redirected last sorting: {redirected_last_sorting:?}");

            pipeline.push(SqlTransform::Sort(redirected_last_sorting));
        }

        Ok(SqlQuery {
            ctes,
            main_relation,
        })
    }
}

impl PqMapper<RelationExpr, RelationExpr, (), ()> for SortingInference<'_> {
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
                                sorting.clone_from(&cte_sorting.sorting);
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
                SqlTransform::Sort(expr) => {
                    sorting.clone_from(&expr);
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
        log::debug!("-- relation sorting {sorting:?}");

        if !self.main_relation {
            // if this is a CTE, make sure that its SELECT includes the
            // columns from the sort
            let select = result.iter_mut().find_map(|x| x.as_select_mut()).unwrap();
            for column_sort in &sorting {
                let cid = column_sort.column;
                let is_selected = select.contains(&cid);
                if !is_selected {
                    log::debug!("adding {cid:?} to {select:?}");
                    select.push(cid);
                }
            }
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

impl RqFold for RelVarNameAssigner<'_> {}

impl PqFold for RelVarNameAssigner<'_> {
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

impl PqMapper<RelationExpr, RelationExpr, (), ()> for RelVarNameAssigner<'_> {
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
