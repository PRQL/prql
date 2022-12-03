use anyhow::Result;
use core::panic;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use crate::ast::pl::TableExternRef;
use crate::ast::rq::{
    self, fold_transform, CId, ColumnDecl, ColumnDeclKind, Expr, ExprKind, IrFold, Relation,
    TableDecl, TableRef, Transform,
};

use super::context::AnchorContext;

type RemainingPipeline = (Vec<Transform>, Vec<CId>);

/// Splits pipeline into two parts, such that the second part contains
/// maximum number of transforms while "fitting" into a SELECT query.
pub fn split_off_back(
    ctx: &mut AnchorContext,
    output: Vec<CId>,
    mut pipeline: Vec<Transform>,
) -> (Option<RemainingPipeline>, Vec<Transform>) {
    if pipeline.is_empty() {
        return (None, Vec::new());
    }

    log::debug!("traversing pipeline to obtain columns: {output:?}");

    let mut following_transforms: HashSet<String> = HashSet::new();

    let mut inputs_required = into_requirements(output, Complexity::highest(), true);
    let mut inputs_avail = HashSet::new();

    // iterate backwards
    let mut curr_pipeline_rev = Vec::new();
    'pipeline: while let Some(transform) = pipeline.pop() {
        // stop if split is needed
        let split = is_split_required(&transform, &mut following_transforms);
        if split {
            log::debug!("split required after {}", transform.as_ref());
            log::debug!(".. following={:?}", following_transforms);
            pipeline.push(transform);
            break;
        }

        // anchor and record all requirements
        let required = get_requirements(&transform, &following_transforms);
        log::debug!("transform {} requires {:?}", transform.as_ref(), required);
        inputs_required.extend(required);

        match &transform {
            Transform::Compute(decl) => {
                if can_materialize(decl, &inputs_required) {
                    log::debug!("materializing {:?}", decl.id);

                    // materialize
                    let col_def = ctx.columns_decls.get_mut(&decl.id).unwrap();
                    col_def.kind = decl.kind.clone();

                    inputs_avail.insert(decl.id);
                } else {
                    pipeline.push(transform);
                    break;
                }
            }
            Transform::Aggregate { compute, .. } => {
                for cid in compute {
                    let decl = &ctx.columns_decls[cid];
                    if !can_materialize(decl, &inputs_required) {
                        pipeline.push(transform);
                        break 'pipeline;
                    }
                }
            }
            Transform::From(with) | Transform::Join { with, .. } => {
                for decl in &with.columns {
                    inputs_avail.insert(decl.id);
                }
            }
            _ => (),
        }

        // push into current pipeline
        if !matches!(transform, Transform::Select(_)) {
            curr_pipeline_rev.push(transform);
        }
    }

    let selected = inputs_required
        .iter()
        .filter(|r| r.selected)
        .map(|r| r.col)
        .collect_vec();

    log::debug!("splitting:");
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
        let selected: Vec<_> = selected.into_iter().unique().collect();

        // Because of s-strings, sometimes, transforms will not have any
        // requirements, which would result in empty SELECTs.
        // As a workaround, let's just fallback to a wildcard.
        let selected = if selected.is_empty() {
            let (input_tables, _) = ctx.collect_pipeline_inputs(&pipeline);

            input_tables
                .iter()
                .map(|tiid| ctx.register_wildcard(*tiid))
                .collect()
        } else {
            selected
        };

        let selected = compress_wildcards(ctx, selected);

        curr_pipeline_rev.push(Transform::Select(selected));
    }

    let remaining_pipeline = if pipeline.is_empty() {
        None
    } else {
        // drop inputs that were satisfied in current pipeline

        Some((pipeline, missing))
    };

    curr_pipeline_rev.reverse();
    (remaining_pipeline, curr_pipeline_rev)
}

fn can_materialize(decl: &ColumnDecl, inputs_required: &[Requirement]) -> bool {
    let complexity = infer_complexity(decl);

    let required_max = inputs_required
        .iter()
        .filter(|r| r.col == decl.id)
        .fold(Complexity::highest(), |c, r| {
            Complexity::min(c, r.max_complexity)
        });

    let can = complexity <= required_max;
    if !can {
        log::debug!(
            "{:?} has complexity {complexity:?}, but is required to have max={required_max:?}",
            decl.id
        );
    }
    can
}

fn compress_wildcards(ctx: &AnchorContext, cols: Vec<CId>) -> Vec<CId> {
    let mut in_wildcard: HashSet<_> = HashSet::new();
    for col in &cols {
        if ctx.columns_decls[col].kind.is_wildcard() {
            if let Some(tiid) = ctx.columns_loc.get(col) {
                in_wildcard.extend(ctx.table_instances[tiid].columns.iter().map(|c| c.id));
            }
        }
    }
    cols.into_iter()
        .filter(|c| !in_wildcard.contains(c) || ctx.columns_decls[c].kind.is_wildcard())
        .collect_vec()
}

/// Applies adjustments to second part of a pipeline when it's split:
/// - prepend pipeline with From
/// - redefine columns materialized in preceding pipeline
/// - redirect all references to original columns to the new ones
pub fn anchor_split(
    ctx: &mut AnchorContext,
    first_table_name: &str,
    cols_at_split: &[CId],
    second_pipeline: Vec<Transform>,
) -> Vec<Transform> {
    let new_tid = ctx.tid.gen();

    log::debug!("split pipeline, first pipeline output: {cols_at_split:?}");

    // define columns of the new CTE
    let mut cid_redirects = HashMap::<CId, CId>::new();
    let mut new_columns = Vec::new();
    for old_cid in cols_at_split {
        let old_def = ctx.columns_decls.get(old_cid).unwrap();

        let kind = match &old_def.kind {
            ColumnDeclKind::Wildcard => ColumnDeclKind::Wildcard,
            ColumnDeclKind::ExternRef(name) => ColumnDeclKind::ExternRef(name.clone()),
            ColumnDeclKind::Expr { .. } => {
                ColumnDeclKind::ExternRef(ctx.ensure_column_name(old_cid))
            }
        };

        let id = ctx.cid.gen();
        let col = ColumnDecl {
            id,
            kind,
            window: None,
            is_aggregation: false,
        };

        new_columns.push(col);
        cid_redirects.insert(*old_cid, id);
    }

    // define a new table
    ctx.table_decls.insert(
        new_tid,
        TableDecl {
            id: new_tid,
            name: Some(first_table_name.to_string()),
            // here we should put the pipeline, but because how this function is called,
            // we need to return the pipeline directly, so we just instert dummy expr instead
            relation: Relation::ExternRef(TableExternRef::LocalTable("".to_string()), vec![]),
        },
    );

    // define instance of that table
    let table_ref = TableRef {
        source: new_tid,
        name: Some(first_table_name.to_string()),
        columns: new_columns,
    };
    ctx.register_table_instance(table_ref.clone());

    // adjust second part: prepend from and rewrite expressions to use new columns
    let mut second = second_pipeline;
    second.insert(0, Transform::From(table_ref));

    let mut redirector = CidRedirector { ctx, cid_redirects };
    redirector.fold_transforms(second).unwrap()
}

/// For the purpose of codegen, TableRef should contain ExternRefs to other
/// tables as if they were actual tables in the database.
/// This function converts TableRef.columns to ExternRefs (or Wildcard)
pub fn materialize_inputs(pipeline: &[Transform], ctx: &mut AnchorContext) {
    let (_, inputs) = ctx.collect_pipeline_inputs(pipeline);
    for cid in inputs {
        let extern_ref = infer_extern_ref(cid, ctx);

        if let Some(extern_ref) = extern_ref {
            let decl = ctx.columns_decls.get_mut(&cid).unwrap();
            decl.kind = extern_ref;
        } else {
            panic!("cannot infer an name for {cid:?}")
        }
    }
}

fn infer_extern_ref(cid: CId, ctx: &AnchorContext) -> Option<ColumnDeclKind> {
    let decl = &ctx.columns_decls[&cid];

    match &decl.kind {
        ColumnDeclKind::Wildcard | ColumnDeclKind::ExternRef(_) => Some(decl.kind.clone()),
        ColumnDeclKind::Expr { name, expr } => {
            if let Some(name) = name {
                Some(ColumnDeclKind::ExternRef(name.clone()))
            } else {
                match &expr.kind {
                    ExprKind::ColumnRef(cid) => infer_extern_ref(*cid, ctx),
                    _ => None,
                }
            }
        }
    }
}

/// Determines whether a pipeline must be split at a transform to
/// fit into one SELECT statement.
///
/// `following` contain names of following transforms in the pipeline.
fn is_split_required(transform: &Transform, following: &mut HashSet<String>) -> bool {
    // Pipeline must be split when there is a transform that is out of order:
    // - from (max 1x),
    // - join (no limit),
    // - filters (for WHERE)
    // - aggregate (max 1x)
    // - filters (for HAVING)
    // - compute (no limit)
    // - sort (no limit)
    // - take (no limit)
    //
    // Select is not affected by the order.
    use Transform::*;

    // Compute for aggregation does not count as a real compute,
    // because it's done within the aggregation
    if let Compute(decl) = transform {
        if decl.is_aggregation {
            return false;
        }
    }

    let split = match transform {
        From(_) => following.contains("From"),
        Join { .. } => following.contains("From"),
        Aggregate { .. } => {
            following.contains("From")
                || following.contains("Join")
                || following.contains("Aggregate")
        }
        Filter(_) => following.contains("From") || following.contains("Join"),
        Compute(_) => {
            following.contains("From")
                || following.contains("Join")
                // || following.contains("Aggregate")
                || following.contains("Filter")
        }
        Sort(_) => {
            following.contains("From")
                || following.contains("Join")
                || following.contains("Compute")
                || following.contains("Aggregate")
        }
        Take(_) => {
            following.contains("From")
                || following.contains("Join")
                || following.contains("Compute")
                || following.contains("Filter")
                || following.contains("Aggregate")
                || following.contains("Sort")
        }

        _ => false,
    };

    if !split {
        following.insert(transform.as_ref().to_string());
    }
    split
}

/// An input requirement of a transform.
struct Requirement {
    pub col: CId,

    /// Maxium complexity with which this column can be expressed in this transform
    pub max_complexity: Complexity,

    /// True iff this column needs to be SELECTed so I can be referenced in this transform
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

fn get_requirements(transform: &Transform, following: &HashSet<String>) -> Vec<Requirement> {
    use Transform::*;

    if let Aggregate { partition, compute } = transform {
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

    let cids = match transform {
        Compute(cd) => match &cd.kind {
            ColumnDeclKind::Expr { expr, .. } => CidCollector::collect(expr.clone()),
            ColumnDeclKind::ExternRef(_) | ColumnDeclKind::Wildcard => return Vec::new(),
        },
        Filter(expr) | Join { filter: expr, .. } => CidCollector::collect(expr.clone()),
        Sort(sorts) => sorts.iter().map(|s| s.column).collect(),
        Take(rq::Take { range, .. }) => {
            let mut cids = Vec::new();
            if let Some(e) = &range.start {
                cids.extend(CidCollector::collect(e.clone()));
            }
            if let Some(e) = &range.end {
                cids.extend(CidCollector::collect(e.clone()));
            }
            cids
        }

        Select(_) | From(_) | Aggregate { .. } | Unique => return Vec::new(),
    };

    let (max_complexity, selected) = match transform {
        Compute(decl) => (
            if infer_complexity(decl) == Complexity::Plain {
                Complexity::Aggregation
            } else {
                Complexity::Plain
            },
            false,
        ),
        Filter(_) => (
            if !following.contains("Aggregate") {
                Complexity::Aggregation
            } else {
                Complexity::Plain
            },
            false,
        ),
        // ORDER BY uses aliased columns, so the columns can have high complexity
        Sort(_) => (Complexity::Aggregation, true),
        Take(_) => (Complexity::Plain, false),
        Join { .. } => (Complexity::Plain, false),

        _ => unreachable!(),
    };

    into_requirements(cids, max_complexity, selected)
}

/// Complexity of a column expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Complexity {
    /// Non-aggregated and non-windowed expressions
    Plain,
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

pub fn infer_complexity(decl: &ColumnDecl) -> Complexity {
    use Complexity::*;

    if decl.window.is_some() {
        Windowed
    } else if decl.is_aggregation {
        Aggregation
    } else {
        Plain
    }
}

#[derive(Default)]
pub struct CidCollector {
    cids: HashSet<CId>,
}

impl CidCollector {
    pub fn collect(expr: Expr) -> Vec<CId> {
        let mut collector = CidCollector::default();
        collector.fold_expr(expr).unwrap();
        collector.cids.into_iter().collect_vec()
    }
}

impl IrFold for CidCollector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        self.cids.insert(cid);
        Ok(cid)
    }
}

struct CidRedirector<'a> {
    ctx: &'a mut AnchorContext,
    cid_redirects: HashMap<CId, CId>,
}

impl<'a> IrFold for CidRedirector<'a> {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(self.cid_redirects.get(&cid).cloned().unwrap_or(cid))
    }

    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        match transform {
            Transform::Compute(cd) => {
                let cd = self.fold_column_decl(cd)?;
                self.ctx.columns_decls.insert(cd.id, cd.clone());
                Ok(Transform::Compute(cd))
            }
            _ => fold_transform(self, transform),
        }
    }
}
