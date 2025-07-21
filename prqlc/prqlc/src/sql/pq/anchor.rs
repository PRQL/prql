use std::collections::{HashMap, HashSet};
use std::fmt;
use std::ops::Deref;

use itertools::Itertools;

use super::ast::{PqMapper, SqlTransform};
use super::context::{AnchorContext, ColumnDecl, RIId, RelationStatus, SqlTableDecl};
use crate::ir::generic::ColumnSort;
use crate::ir::rq::{
    self, fold_column_sorts, fold_transform, CId, Compute, Expr, RelationColumn, RqFold, TableRef,
    Transform,
};
use crate::sql::pq::context::RelationAdapter;
use crate::sql::pq::positional_mapping::compute_positional_mappings;
use crate::Result;

/// Extract last part of pipeline that is able to "fit" into a single SELECT statement.
/// Remaining proceeding pipeline is declared as a table and stored in AnchorContext.
pub(super) fn extract_atomic(
    pipeline: Vec<SqlTransform>,
    ctx: &mut AnchorContext,
) -> Vec<SqlTransform> {
    let output = ctx.determine_select_columns(&pipeline);
    let output = ctx.positional_mapping.apply_active_mapping(output);

    let (preceding, atomic) = split_off_back(pipeline, output.clone(), ctx);

    let atomic = if let Some(preceding) = preceding {
        log::debug!(
            "pipeline split after {}",
            preceding.last().unwrap().as_str()
        );
        anchor_split(ctx, preceding, atomic)
    } else {
        atomic
    };

    // sometimes, additional columns will be added into select, because they are needed for
    // other clauses. To filter them out, we use an additional limiting SELECT.
    let output: Vec<_> = CidRedirector::redirect_cids(output, &atomic, ctx);
    let select_cols = atomic
        .iter()
        .find_map(|x| x.as_super().and_then(|y| y.as_select()))
        .unwrap();
    if select_cols.iter().any(|c| !output.contains(c)) {
        log::debug!(
            "appending a projection SELECT, because previous one contained un-selected columns"
        );

        // duplicate Select for purposes of anchor_split
        let duplicated_select = SqlTransform::Super(Transform::Select(select_cols.clone()));
        let mut atomic = atomic;
        atomic.push(duplicated_select);

        // construct the new SELECT
        let limited_view = vec![SqlTransform::Super(Transform::Select(output))];

        return anchor_split(ctx, atomic, limited_view);
    }

    atomic
}

/// Splits pipeline into two parts, such that the second part contains
/// maximum number of transforms while "fitting" into a SELECT query.
///
/// Returns optional remaining preceding pipeline and the atomic pipeline.
pub(super) fn split_off_back(
    mut pipeline: Vec<SqlTransform>,
    output: Vec<CId>,
    ctx: &mut AnchorContext,
) -> (Option<Vec<SqlTransform>>, Vec<SqlTransform>) {
    if pipeline.is_empty() {
        return (None, Vec::new());
    }

    let mapping_before = compute_positional_mappings(&pipeline);

    log::debug!("traversing pipeline to obtain columns: {output:?}");

    let mut following_transforms: HashSet<String> = HashSet::new();

    let mut inputs_required = Requirements::from_cids(output.iter())
        .allow_up_to(Complexity::highest())
        .should_select(true);

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
        log::debug!(".. transform {} requires {required:?}", transform.as_str(),);
        inputs_required = inputs_required.append(required.clone());

        match &transform {
            SqlTransform::Super(Transform::Compute(compute)) => {
                let (can_mat, max_complexity) = can_materialize(compute, &inputs_required);
                if can_mat {
                    log::debug!("materializing {:?}", compute.id);
                    inputs_avail.insert(compute.id);

                    // add transitive dependencies
                    inputs_required = inputs_required
                        .append(required.allow_up_to(max_complexity).should_select(false));
                } else {
                    pipeline.push(transform);
                    break;
                }
            }
            SqlTransform::Super(Transform::Aggregate { compute, .. }) => {
                for cid in compute {
                    let decl = &ctx.column_decls[cid];
                    if let ColumnDecl::Compute(compute) = decl {
                        if !can_materialize(compute, &inputs_required).0 {
                            pipeline.push(transform);
                            break 'pipeline;
                        }
                    }
                }
            }
            SqlTransform::From(with) | SqlTransform::Join { with, .. } => {
                let relation = ctx.relation_instances.get_mut(with).unwrap();
                for (_, cid) in &relation.table_ref.columns {
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

    let required = inputs_required.iter().map(|r| r.col).unique().collect_vec();
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

    // This will compare columns for order sensitive transform and correct it in subsequent relation.
    let mapping_after = compute_positional_mappings(&curr_pipeline_rev);
    for (before, after) in mapping_before.iter().zip(mapping_after.iter()) {
        ctx.positional_mapping
            .compute_and_store_mapping(before, after);
    }

    (remaining_pipeline, curr_pipeline_rev)
}

fn can_materialize(compute: &Compute, inputs_required: &[Requirement]) -> (bool, Complexity) {
    let complexity = infer_complexity(compute);

    let required = inputs_required
        .iter()
        .filter(|r| r.col == compute.id)
        .fold(Complexity::highest(), |c, r| {
            Complexity::min(c, r.max_complexity)
        });

    let can_materialize = complexity <= required;
    if !can_materialize {
        // cannot materialize here, complexity is greater than what's required here
        log::debug!(
            "{:?} has complexity {complexity:?}, but is required to have at most {required:?}",
            compute.id
        );
    }
    (can_materialize, required)
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
) -> Vec<SqlTransform> {
    let new_tid = ctx.tid.gen();

    let preceding_select = &preceding.last().unwrap().as_super().unwrap();
    let cols_at_split = preceding_select.as_select().unwrap();

    log::debug!("split pipeline, first pipeline output: {cols_at_split:?}");

    // redefine columns of the atomic pipeline
    let mut cid_redirects = HashMap::<CId, CId>::new();
    let mut new_columns = Vec::new();
    let mut used_new_names = HashSet::new();
    for old_cid in cols_at_split {
        let new_cid = ctx.cid.gen();

        let old_name = ctx.ensure_column_name(*old_cid).cloned();

        let mut new_name = old_name;
        if let Some(new) = &mut new_name {
            if used_new_names.contains(new) {
                *new = ctx.col_name.gen();
                ctx.column_names.insert(*old_cid, new.clone());
            }

            used_new_names.insert(new.clone());
            ctx.column_names.insert(new_cid, new.clone());
        }

        let old_def = ctx.column_decls.get(old_cid).unwrap();

        let col = match old_def {
            ColumnDecl::RelationColumn(_, _, RelationColumn::Wildcard) => RelationColumn::Wildcard,
            _ => RelationColumn::Single(new_name),
        };

        new_columns.push((col, new_cid));
        cid_redirects.insert(*old_cid, new_cid);
    }

    // define a new table
    let columns = cols_at_split
        .iter()
        .map(|_| RelationColumn::Single(None))
        .collect_vec();
    ctx.table_decls.insert(
        new_tid,
        SqlTableDecl {
            id: new_tid,
            name: None,
            relation: RelationStatus::NotYetDefined(RelationAdapter::Preprocessed(
                preceding, columns,
            )),
            redirect_to: None,
        },
    );

    // define instance of that table
    let riid = ctx.create_relation_instance(
        TableRef {
            source: new_tid,
            name: None,
            columns: new_columns,
            prefer_cte: true,
        },
        cid_redirects,
    );

    // adjust second part: prepend from and rewrite expressions to use new columns
    let mut second = atomic;
    second.insert(0, SqlTransform::From(riid));

    CidRedirector::redirect_pipeline(second, ctx)
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
    use SqlTransform::Super;
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
        SqlTransform::From(_) => contains_any(following, ["From"]),
        SqlTransform::Join { .. } => contains_any(following, ["From"]),
        Super(Aggregate { .. }) => {
            contains_any(following, ["From", "Join", "Aggregate", "Compute"])
        }
        Super(Filter(_)) => contains_any(following, ["From", "Join"]),
        Super(Compute(_)) => contains_any(following, ["From", "Join", /* "Aggregate" */ "Filter"]),

        // Sort will be pushed down the CTEs, so there is no point in splitting for it.
        // Super(Sort(_)) => contains_any(following, ["From", "Join", "Compute", "Aggregate"]),
        Super(Take(_)) => contains_any(
            following,
            ["From", "Join", "Compute", "Filter", "Aggregate", "Sort"],
        ),
        SqlTransform::DistinctOn(_) => contains_any(
            following,
            [
                "From",
                "Join",
                "Compute",
                "Filter",
                "Aggregate",
                "Sort",
                "Take",
                "DistinctOn",
            ],
        ),
        SqlTransform::Distinct => contains_any(
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
        SqlTransform::Union { .. }
        | SqlTransform::Except { .. }
        | SqlTransform::Intersect { .. } => contains_any(
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
        Super(Loop(_)) => !following.is_empty(),
        _ => false,
    };

    if !split {
        following.insert(transform.as_str().to_string());
    }
    split
}

/// An input requirement of a transform.
#[derive(Clone)]
pub struct Requirement {
    pub col: CId,

    /// Maximum complexity with which this column can be expressed in this transform
    pub max_complexity: Complexity,

    /// True iff this column needs to be SELECTed so it can be referenced in this transform
    pub selected: bool,
}

#[derive(Clone, Default)]
pub struct Requirements(Vec<Requirement>);

/// To iter on `Requirements` as if it's a simple `Vec`.
impl Deref for Requirements {
    type Target = [Requirement];

    fn deref(&self) -> &[Requirement] {
        self.0.as_slice()
    }
}

/// To debug `Requirements` as if it's a simple `Vec`.
impl fmt::Debug for Requirements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Requirements {
    /// Turns a list of `CId` into requirements with the least allowed complexity
    /// and unselected by default.
    pub fn from_cids<'a, I>(cids: I) -> Requirements
    where
        I: Iterator<Item = &'a CId>,
    {
        Requirements(
            cids.cloned()
                .map(|col| Requirement {
                    col,
                    max_complexity: Complexity::lowest(),
                    selected: false,
                })
                .collect(),
        )
    }

    /// Collect columns from the given `Expr` into requirements with
    /// the least allowed complexity and unselected by default.
    pub fn from_expr(expr: &Expr) -> Requirements {
        let cids = CidCollector::collect(expr.clone());
        Requirements::from_cids(cids.iter())
    }

    /// Moves all the elements of `other` into `self`, leaving `other` empty,
    /// then return `self` for chainability.
    pub fn append(mut self, mut other: Requirements) -> Requirements {
        self.0.append(&mut other.0);
        self
    }

    /// Set a maximum complexity to all stored requirements.
    pub fn allow_up_to(mut self, max_complexity: Complexity) -> Self {
        for r in &mut self.0 {
            r.max_complexity = max_complexity;
        }
        self
    }

    /// Set a SELECT status to all stored requirements.
    pub fn should_select(mut self, selected: bool) -> Self {
        for r in &mut self.0 {
            r.selected = selected;
        }
        self
    }
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
) -> Requirements {
    use SqlTransform::Super;

    match transform {
        Super(Transform::Aggregate { partition, compute }) => {
            let partition_requirements = Requirements::from_cids(partition.iter());
            let compute_requirements =
                Requirements::from_cids(compute.iter()).allow_up_to(Complexity::Aggregation);

            partition_requirements.append(compute_requirements)
        }

        Super(Transform::Compute(compute)) => {
            let requirements = Requirements::from_expr(&compute.expr).allow_up_to(
                match infer_complexity(compute) {
                    // plain expressions can be included in anything less complex than Aggregation
                    Complexity::Plain => Complexity::Aggregation,

                    // anything more complex can only use included in other plain expressions.
                    // in other words: complex expressions (aggregation, window functions) cannot
                    // be defined within other expressions.
                    _ => Complexity::Plain,
                },
            );

            if let Some(window) = &compute.window {
                // TODO: what kind of exprs can be in window frame?
                // window.frame

                let window_cids = window
                    .partition
                    .iter()
                    .chain(window.sort.iter().map(|s| &s.column));

                requirements.append(Requirements::from_cids(window_cids))
            } else {
                requirements
            }
        }

        Super(Transform::Filter(expr)) => {
            Requirements::from_expr(expr).allow_up_to(if !following.contains("Aggregate") {
                Complexity::Aggregation
            } else {
                Complexity::Plain
            })
        }

        // Aggregations require that all selected columns be wrapped in aggregate functions (e.g., SUM, COUNT).
        Super(Transform::Sort(sorts)) if !following.contains("Aggregate") => {
            Requirements::from_cids(sorts.iter().map(|s| &s.column))
                // we only use SELECTed columns in ORDER BY, so the columns can have high complexity
                .allow_up_to(Complexity::Aggregation)
                .should_select(true)
        }

        SqlTransform::DistinctOn(partition) => Requirements::from_cids(partition.iter())
            // Since there is aggregation anyway, columns can have any complexity
            .allow_up_to(Complexity::highest()),

        Super(Transform::Take(rq::Take { range, .. })) => [&range.start, &range.end]
            .into_iter()
            .flatten()
            .map(Requirements::from_expr)
            .fold(Requirements::default(), Requirements::append),

        SqlTransform::Join { filter, .. } => Requirements::from_expr(filter),

        _ => Requirements::default(),
    }
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
    const fn lowest() -> Self {
        Self::Plain
    }

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
        rq::ExprKind::Operator { args, .. } => args
            .iter()
            .map(infer_complexity_expr)
            .max()
            .unwrap_or(Complexity::Plain),
        rq::ExprKind::ColumnRef(_)
        | rq::ExprKind::Literal(_)
        | rq::ExprKind::SString(_)
        | rq::ExprKind::Param(_) => Complexity::Plain,
        rq::ExprKind::Array(elements) => elements
            .iter()
            .map(infer_complexity_expr)
            .max()
            .unwrap_or(Complexity::Plain),
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
    ctx: &'a mut AnchorContext,
    cid_redirects: HashMap<CId, CId>,
}

impl<'a> CidRedirector<'a> {
    pub fn of_first_from(pipeline: &[SqlTransform], ctx: &'a mut AnchorContext) -> Option<Self> {
        let from = pipeline.first()?.as_from()?;
        let relation_instance = &ctx.relation_instances[from];
        let cid_redirects = relation_instance.cid_redirects.clone();
        Some(CidRedirector { ctx, cid_redirects })
    }

    pub fn redirect_pipeline(
        pipeline: Vec<SqlTransform>,
        ctx: &'a mut AnchorContext,
    ) -> Vec<SqlTransform> {
        let Some(mut redirector) = Self::of_first_from(&pipeline, ctx) else {
            return pipeline;
        };

        redirector.fold_sql_transforms(pipeline).unwrap()
    }

    /// Redirects cids within a context of a pipeline.
    /// This will find cid_redirects of the first From in the pipeline.
    pub fn redirect_cids(
        cids: Vec<CId>,
        pipeline: &[SqlTransform],
        ctx: &'a mut AnchorContext,
    ) -> Vec<CId> {
        // find cid_redirects
        let Some(mut redirector) = Self::of_first_from(pipeline, ctx) else {
            return cids;
        };
        redirector.fold_cids(cids).unwrap()
    }

    pub fn redirect_sorts(
        sorts: Vec<ColumnSort<CId>>,
        riid: &RIId,
        ctx: &'a mut AnchorContext,
    ) -> Vec<ColumnSort<CId>> {
        let cid_redirects = ctx.relation_instances[riid].cid_redirects.clone();
        log::debug!("redirect sorts {sorts:?} {riid:?} cid_redirects {cid_redirects:?}");
        let mut redirector = CidRedirector { ctx, cid_redirects };

        fold_column_sorts(&mut redirector, sorts).unwrap()
    }
}

impl RqFold for CidRedirector<'_> {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        let v = self.cid_redirects.get(&cid).cloned().unwrap_or(cid);
        log::debug!("mapping {cid:?} via {0:?} to {v:?}", self.cid_redirects);
        Ok(v)
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

impl PqMapper<RIId, RIId, Transform, Transform> for CidRedirector<'_> {
    fn fold_rel(&mut self, rel: RIId) -> Result<RIId> {
        Ok(rel)
    }

    fn fold_super(&mut self, sup: Transform) -> Result<Transform> {
        self.fold_transform(sup)
    }
}
