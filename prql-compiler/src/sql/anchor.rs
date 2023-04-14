use anyhow::Result;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use crate::ast::rq::{
    self, fold_transform, CId, Compute, Expr, RelationColumn, RqFold, TableRef, Transform,
};
use crate::sql::ast_srq::{SqlRelation, SqlRelationKind};
use crate::sql::context::SqlTableDecl;

use super::ast_srq::{SqlFold, SqlTransform};
use super::context::{AnchorContext, ColumnDecl};

/// Extract last part of pipeline that is able to "fit" into a single SELECT statement.
/// Remaining proceeding pipeline is declared as a table and stored in AnchorContext.
pub(super) fn extract_atomic(
    pipeline: Vec<SqlTransform>,
    ctx: &mut AnchorContext,
) -> Vec<SqlTransform> {
    let output = AnchorContext::determine_select_columns(&pipeline);

    let (preceding, atomic) = split_off_back(pipeline, output.clone(), ctx);

    let (atomic, cid_redirects) = if let Some(preceding) = preceding {
        log::debug!(
            "pipeline split after {}",
            preceding.last().unwrap().as_str()
        );
        anchor_split(ctx, preceding, atomic)
    } else {
        (atomic, HashMap::new())
    };

    // sometimes, additional columns will be added into select, because they are needed for
    // other clauses. To filter them out, we use an additional limiting SELECT.
    let output: Vec<_> = output
        .into_iter()
        .map(|c| cid_redirects.get(&c).cloned().unwrap_or(c))
        .collect();
    let select_cols = atomic
        .iter()
        .find_map(|x| x.as_super().and_then(|y| y.as_select()))
        .unwrap();
    if select_cols.iter().any(|c| !output.contains(c)) {
        // duplicate Select for purposes of anchor_split
        let duplicated_select = SqlTransform::Super(Transform::Select(select_cols.clone()));
        let mut atomic = atomic;
        atomic.push(duplicated_select);

        // construct the new SELECT
        let limited_view = vec![SqlTransform::Super(Transform::Select(output))];

        let (limited_view, _) = anchor_split(ctx, atomic, limited_view);
        return limited_view;
    }

    atomic
}

/// Converts a pipeline into atomic pipelines. Meant for debugging purposes.
pub(super) fn extract_atomics_naive(
    pipeline: Vec<SqlTransform>,
    ctx: &mut AnchorContext,
) -> Vec<Vec<SqlTransform>> {
    let mut atomics = Vec::new();

    atomics.push(extract_atomic(pipeline, ctx));
    while let Some(decl_key) = ctx.table_decls.keys().max_by_key(|t| t.get()).cloned() {
        let decl = ctx.table_decls.remove(&decl_key).unwrap();
        if let Some(relation) = decl.relation {
            if let SqlRelationKind::PreprocessedPipeline(p) = relation.kind {
                atomics.push(extract_atomic(p, ctx));
            }
        }
    }

    atomics.reverse();
    atomics
}

/// Splits pipeline into two parts, such that the second part contains
/// maximum number of transforms while "fitting" into a SELECT query.
pub(super) fn split_off_back(
    mut pipeline: Vec<SqlTransform>,
    output: Vec<CId>,
    ctx: &mut AnchorContext,
) -> (Option<Vec<SqlTransform>>, Vec<SqlTransform>) {
    if pipeline.is_empty() {
        return (None, Vec::new());
    }

    log::debug!("traversing pipeline to obtain columns: {output:?}");

    let mut following_transforms: HashSet<String> = HashSet::new();

    let mut inputs_required = into_requirements(output.clone(), Complexity::highest(), true);
    let mut inputs_avail = HashSet::new();

    // iterate backwards
    let mut curr_pipeline_rev = Vec::new();
    'pipeline: while let Some(transform) = pipeline.pop() {
        // stop if split is needed
        let split = is_split_required(&transform, &mut following_transforms);
        if split {
            log::debug!("split required after {}", transform.as_str());
            log::debug!(".. following={:?}", following_transforms);
            pipeline.push(transform);
            break;
        }

        // anchor and record all requirements
        let required = get_requirements(&transform, &following_transforms);
        log::debug!("transform {} requires {:?}", transform.as_str(), required);
        inputs_required.extend(required);

        match &transform {
            SqlTransform::Super(Transform::Compute(compute)) => {
                if can_materialize(compute, &inputs_required) {
                    log::debug!("materializing {:?}", compute.id);
                    inputs_avail.insert(compute.id);
                } else {
                    pipeline.push(transform);
                    break;
                }
            }
            SqlTransform::Super(Transform::Aggregate { compute, .. }) => {
                for cid in compute {
                    let decl = &ctx.column_decls[cid];
                    if let ColumnDecl::Compute(compute) = decl {
                        if !can_materialize(compute, &inputs_required) {
                            pipeline.push(transform);
                            break 'pipeline;
                        }
                    }
                }
            }
            SqlTransform::Super(Transform::From(with) | Transform::Join { with, .. }) => {
                for (_, cid) in &with.columns {
                    inputs_avail.insert(*cid);
                }
            }
            _ => (),
        }

        // push into current pipeline
        if !matches!(transform, SqlTransform::Super(Transform::Select(_))) {
            curr_pipeline_rev.push(transform);
        }
    }

    let selected = inputs_required
        .iter()
        .filter(|r| r.selected)
        .map(|r| r.col)
        .collect_vec();

    log::debug!("finished table:");
    log::debug!(".. avail={inputs_avail:?}");

    let required = inputs_required
        .into_iter()
        .map(|r| r.col)
        .unique()
        .collect_vec();
    log::debug!(".. required={required:?}");

    let missing = required
        .into_iter()
        .filter(|i| !inputs_avail.contains(i))
        .collect_vec();
    log::debug!(".. missing={missing:?}");

    // figure out SELECT columns
    {
        // output cols must preserve duplicates, but selected inputs has to be deduplicated
        let mut output = output;
        for c in selected {
            if !output.contains(&c) {
                output.push(c);
            }
        }

        // Because of s-strings, sometimes, transforms will not have any
        // requirements, which would result in empty SELECTs.
        // As a workaround, let's just fallback to a wildcard.
        let output = if output.is_empty() {
            let (input_tables, _) = ctx.collect_pipeline_inputs(&pipeline);

            input_tables
                .iter()
                .map(|tiid| ctx.register_wildcard(*tiid))
                .collect()
        } else {
            output
        };

        curr_pipeline_rev.push(SqlTransform::Super(Transform::Select(output)));
    }

    let remaining_pipeline = if pipeline.is_empty() {
        None
    } else {
        // drop inputs that were satisfied in current pipeline
        pipeline.push(SqlTransform::Super(Transform::Select(missing)));

        Some(pipeline)
    };

    curr_pipeline_rev.reverse();
    (remaining_pipeline, curr_pipeline_rev)
}

fn can_materialize(compute: &Compute, inputs_required: &[Requirement]) -> bool {
    let complexity = infer_complexity(compute);

    let required_max = inputs_required
        .iter()
        .filter(|r| r.col == compute.id)
        .fold(Complexity::highest(), |c, r| {
            Complexity::min(c, r.max_complexity)
        });

    let can = complexity <= required_max;
    if !can {
        log::debug!(
            "{:?} has complexity {complexity:?}, but is required to have max={required_max:?}",
            compute.id
        );
    }
    can
}

/// Applies adjustments to second part of a pipeline when it's split:
/// - append Select to proceeding pipeline
/// - prepend From to atomic pipeline
/// - redefine columns materialized in atomic pipeline
/// - redirect all references to original columns to the new ones
pub(super) fn anchor_split(
    ctx: &mut AnchorContext,
    preceding: Vec<SqlTransform>,
    atomic: Vec<SqlTransform>,
) -> (Vec<SqlTransform>, HashMap<CId, CId>) {
    let new_tid = ctx.tid.gen();

    let preceding_select = &preceding.last().unwrap().as_super().unwrap();
    let cols_at_split = preceding_select.as_select().unwrap();

    log::debug!("split pipeline, first pipeline output: {cols_at_split:?}");

    // redefine columns of the atomic pipeline
    let mut cid_redirects = HashMap::<CId, CId>::new();
    let mut new_columns = Vec::new();
    for old_cid in cols_at_split {
        let new_cid = ctx.cid.gen();

        let old_name = ctx.ensure_column_name(*old_cid).cloned();
        if let Some(name) = old_name.clone() {
            ctx.column_names.insert(new_cid, name);
        }

        let old_def = ctx.column_decls.get(old_cid).unwrap();

        let col = match old_def {
            ColumnDecl::RelationColumn(_, _, RelationColumn::Wildcard) => RelationColumn::Wildcard,
            _ => RelationColumn::Single(old_name),
        };

        new_columns.push((col, new_cid));
        cid_redirects.insert(*old_cid, new_cid);
    }

    // define a new table
    ctx.table_decls.insert(
        new_tid,
        SqlTableDecl {
            id: new_tid,
            name: None,
            relation: Some(SqlRelation {
                columns: cols_at_split
                    .iter()
                    .map(|_| RelationColumn::Single(None))
                    .collect_vec(),
                kind: SqlRelationKind::PreprocessedPipeline(preceding),
            }),
        },
    );

    // define instance of that table
    let table_ref = ctx.create_table_instance(TableRef {
        source: new_tid,
        name: None,
        columns: new_columns,
    });

    // adjust second part: prepend from and rewrite expressions to use new columns
    let mut second = atomic;
    second.insert(0, SqlTransform::Super(Transform::From(table_ref)));

    let second = CidRedirector::redirect(second, &cid_redirects, ctx);

    (second, cid_redirects)
}

/// Determines whether a pipeline must be split at a transform to
/// fit into one SELECT statement.
///
/// `following` contain names of following transforms in the pipeline.
fn is_split_required(transform: &SqlTransform, following: &mut HashSet<String>) -> bool {
    // Pipeline must be split when there is a transform that is out of order:
    // - from (max 1x),
    // - join (no limit),
    // - filters (for WHERE)
    // - aggregate (max 1x)
    // - filters (for HAVING)
    // - compute (no limit)
    // - sort (no limit)
    // - take (no limit)
    // - distinct
    // - append/except/intersect (no limit)
    // - loop (max 1x)
    //
    // Select is not affected by the order.
    use SqlTransform::*;
    use Transform::*;

    // Compute for aggregation does not count as a real compute,
    // because it's done within the aggregation
    if let Super(Compute(decl)) = transform {
        if decl.is_aggregation {
            return false;
        }
    }

    fn contains_any<const C: usize>(set: &HashSet<String>, elements: [&'static str; C]) -> bool {
        for t in elements {
            if set.contains(t) {
                return true;
            }
        }
        false
    }

    let split = match transform {
        Super(From(_)) => contains_any(following, ["From"]),
        Super(Join { .. }) => contains_any(following, ["From"]),
        Super(Aggregate { .. }) => {
            contains_any(following, ["From", "Join", "Aggregate", "Compute"])
        }
        Super(Filter(_)) => contains_any(following, ["From", "Join"]),
        Super(Compute(_)) => contains_any(following, ["From", "Join", /* "Aggregate" */ "Filter"]),
        Super(Sort(_)) => contains_any(following, ["From", "Join", "Compute", "Aggregate"]),
        Super(Take(_)) => contains_any(
            following,
            ["From", "Join", "Compute", "Filter", "Aggregate", "Sort"],
        ),
        Distinct => contains_any(
            following,
            [
                "From",
                "Join",
                "Compute",
                "Filter",
                "Aggregate",
                "Sort",
                "Take",
            ],
        ),
        Union { .. } | Except { .. } | Intersect { .. } => contains_any(
            following,
            [
                "From",
                "Join",
                "Compute",
                "Filter",
                "Aggregate",
                "Sort",
                "Take",
                "Distinct",
            ],
        ),
        SqlTransform::Loop(_) => !following.is_empty(),
        _ => false,
    };

    if !split {
        following.insert(transform.as_str().to_string());
    }
    split
}

/// An input requirement of a transform.
pub struct Requirement {
    pub col: CId,

    /// Maximum complexity with which this column can be expressed in this transform
    pub max_complexity: Complexity,

    /// True iff this column needs to be SELECTed so it can be referenced in this transform
    pub selected: bool,
}

fn into_requirements(
    cids: Vec<CId>,
    max_complexity: Complexity,
    selected: bool,
) -> Vec<Requirement> {
    cids.into_iter()
        .map(|col| Requirement {
            col,
            max_complexity,
            selected,
        })
        .collect()
}

impl std::fmt::Debug for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.col, f)?;
        f.write_str("-as-")?;
        std::fmt::Debug::fmt(&self.max_complexity, f)
    }
}

pub(super) fn get_requirements(
    transform: &SqlTransform,
    following: &HashSet<String>,
) -> Vec<Requirement> {
    use SqlTransform::*;
    use Transform::*;

    // special case for aggregate, which contain two difference Complexities
    if let Super(Aggregate { partition, compute }) = transform {
        let mut r = Vec::new();
        r.extend(into_requirements(
            partition.clone(),
            Complexity::Plain,
            false,
        ));
        r.extend(into_requirements(
            compute.clone(),
            Complexity::Aggregation,
            false,
        ));
        return r;
    }

    // general case: extract cids
    let cids = match transform {
        Super(Compute(compute)) => CidCollector::collect(compute.expr.clone()),
        Super(Filter(expr) | Join { filter: expr, .. }) => CidCollector::collect(expr.clone()),
        Super(Sort(sorts)) => sorts.iter().map(|s| s.column).collect(),
        Super(Take(rq::Take { range, .. })) => {
            let mut cids = Vec::new();
            if let Some(e) = &range.start {
                cids.extend(CidCollector::collect(e.clone()));
            }
            if let Some(e) = &range.end {
                cids.extend(CidCollector::collect(e.clone()));
            }
            cids
        }

        Super(Aggregate { .. } | Append(_) | Transform::Loop(_)) => unreachable!(),
        Super(Select(_) | From(_))
        | Distinct
        | Union { .. }
        | Except { .. }
        | Intersect { .. }
        | SqlTransform::Loop(_) => return Vec::new(),
    };

    // general case: determine complexity
    let (max_complexity, selected) = match transform {
        Super(Compute(decl)) => (
            if infer_complexity(decl) == Complexity::Plain {
                Complexity::Aggregation
            } else {
                Complexity::Plain
            },
            false,
        ),
        Super(Filter(_)) => (
            if !following.contains("Aggregate") {
                Complexity::Aggregation
            } else {
                Complexity::Plain
            },
            false,
        ),
        // we only use aliased columns in ORDER BY, so the columns can have high complexity
        Super(Sort(_)) => (Complexity::Aggregation, true),
        Super(Take(_)) => (Complexity::Plain, false),
        Super(Join { .. }) => (Complexity::Plain, false),

        _ => unreachable!(),
    };

    into_requirements(cids, max_complexity, selected)
}

/// Complexity of a column expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Complexity {
    /// Simple non-aggregated and non-windowed expressions
    Plain,
    /// Expressions that cannot be used in GROUP BY (CASE)
    NonGroup,
    /// Non-aggregated expressions
    Windowed,
    /// Everything
    Aggregation,
}

impl Complexity {
    const fn highest() -> Self {
        Self::Aggregation
    }
}

pub fn infer_complexity(compute: &Compute) -> Complexity {
    use Complexity::*;

    if compute.window.is_some() {
        Windowed
    } else if compute.is_aggregation {
        Aggregation
    } else {
        infer_complexity_expr(&compute.expr)
    }
}

pub fn infer_complexity_expr(expr: &Expr) -> Complexity {
    match &expr.kind {
        rq::ExprKind::Case(_) => Complexity::NonGroup,
        rq::ExprKind::Binary { left, right, .. } => {
            Complexity::max(infer_complexity_expr(left), infer_complexity_expr(right))
        }
        rq::ExprKind::Unary { expr, .. } => infer_complexity_expr(expr),
        rq::ExprKind::BuiltInFunction { args, .. } => args
            .iter()
            .map(infer_complexity_expr)
            .max()
            .unwrap_or(Complexity::Plain),
        rq::ExprKind::ColumnRef(_)
        | rq::ExprKind::Literal(_)
        | rq::ExprKind::SString(_)
        | rq::ExprKind::Param(_)
        | rq::ExprKind::FString(_) => Complexity::Plain,
    }
}

#[derive(Default)]
pub struct CidCollector {
    // we could use HashSet instead of Vec, but this caused nondeterministic
    // results downstream
    cids: Vec<CId>,
}

impl CidCollector {
    pub fn collect(expr: Expr) -> Vec<CId> {
        let mut collector = CidCollector::default();
        collector.fold_expr(expr).unwrap();
        collector.cids
    }

    pub fn collect_t(t: Transform) -> (Transform, Vec<CId>) {
        let mut collector = CidCollector::default();
        let t = collector.fold_transform(t).unwrap();
        (t, collector.cids)
    }
}

impl RqFold for CidCollector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        self.cids.push(cid);
        Ok(cid)
    }
}

pub(super) struct CidRedirector<'a> {
    pub ctx: &'a mut AnchorContext,
    pub cid_redirects: &'a HashMap<CId, CId>,
}

impl<'a> CidRedirector<'a> {
    pub fn redirect(
        pipeline: Vec<SqlTransform>,
        cid_redirects: &'a HashMap<CId, CId>,
        ctx: &mut AnchorContext,
    ) -> Vec<SqlTransform> {
        let mut redirector = CidRedirector { ctx, cid_redirects };
        redirector.fold_sql_transforms(pipeline).unwrap()
    }
}

impl<'a> RqFold for CidRedirector<'a> {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(self.cid_redirects.get(&cid).cloned().unwrap_or(cid))
    }

    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        match transform {
            Transform::Compute(compute) => {
                let compute = self.fold_compute(compute)?;
                self.ctx.register_compute(compute.clone());
                Ok(Transform::Compute(compute))
            }
            _ => fold_transform(self, transform),
        }
    }
}

impl<'a> SqlFold for CidRedirector<'a> {}
