use anyhow::Result;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use crate::{
    ast::TableRef,
    ir::{CId, ColumnDef, ColumnDefKind, Expr, ExprKind, IrFold, TableExpr, Transform},
};

use super::context::{AnchorContext, TableDef};

type RemainingPipeline = (Vec<Transform>, Vec<CId>);

/// Splits pipeline into two parts, such that the second part contains
/// maximum number of transforms while "fitting" into a SELECT query.
pub fn split_off_back(
    context: &mut AnchorContext,
    mut output_cols: Vec<CId>,
    mut pipeline: Vec<Transform>,
) -> (Option<RemainingPipeline>, Vec<Transform>) {
    if pipeline.is_empty() {
        return (None, Vec::new());
    }

    let mut following_transforms: HashSet<String> = HashSet::new();

    let (input_tables, input_columns) = context.collect_pipeline_inputs(&pipeline);
    let inputs_avail = extend_wildcards(context, input_columns);
    let mut inputs_required = Vec::new();

    // iterate backwards
    let mut curr_pipeline_rev = Vec::new();
    while let Some(transform) = pipeline.pop() {
        // stop if split is needed
        let split = is_split_required(&transform, &following_transforms);
        if split {
            pipeline.push(transform);
            break;
        }
        following_transforms.insert(transform.as_ref().to_string());

        // anchor and record all requirements
        let required = get_requirements(&transform);
        for r in required {
            let r_inputs = anchor_column(context, r, &inputs_avail);

            output_cols.extend(&r_inputs - &inputs_avail);
            inputs_required.extend(r_inputs);
        }

        // push into current pipeline
        if !matches!(transform, Transform::Compute(_) | Transform::Select(_)) {
            curr_pipeline_rev.push(transform);
        }
    }

    // prevent finishing if there are still missing requirements
    let has_all_inputs = inputs_required.iter().all(|c| inputs_avail.contains(c));
    if !has_all_inputs && pipeline.is_empty() {
        // push From back to the remaining pipeline
        if let Some(transform) = curr_pipeline_rev.pop() {
            if let Transform::From(_) = &transform {
                pipeline.push(transform);
            } else {
                panic!("pipeline does not start with From!");
            }
        } else {
            unreachable!();
        }
    }

    // figure out SELECT columns
    {
        let cols: Vec<_> = output_cols.into_iter().unique().collect();

        // Because of s-strings, sometimes, transforms will not have any
        // requirements, which would result in empty SELECTs.
        // As a workaround, let's just fallback to a wildcard.
        let cols = if cols.is_empty() {
            input_tables
                .iter()
                .map(|tid| {
                    let id = context.ids.gen_cid();
                    let def = ColumnDef {
                        id,
                        kind: ColumnDefKind::Wildcard(*tid),
                    };
                    context.columns_defs.insert(id, def);
                    context.columns_loc.insert(id, *tid);
                    id
                })
                .collect()
        } else {
            cols
        };

        curr_pipeline_rev.push(Transform::Select(cols));
    }

    let remaining_pipeline = if pipeline.is_empty() {
        None
    } else {
        // drop inputs that were satisfied in current pipeline
        let (_, inputs_in_curr) = context.collect_pipeline_inputs(&curr_pipeline_rev);
        let inputs_remaining = inputs_required
            .into_iter()
            .filter(|i| !inputs_in_curr.contains(i))
            .collect();

        Some((pipeline, inputs_remaining))
    };

    curr_pipeline_rev.reverse();
    (remaining_pipeline, curr_pipeline_rev)
}

fn extend_wildcards(context: &AnchorContext, mut cols: HashSet<CId>) -> HashSet<CId> {
    let wildcard_tables: HashSet<_> = cols
        .iter()
        .filter_map(|cid| context.columns_defs[cid].kind.as_wildcard().cloned())
        .collect();

    for (cid, tid) in &context.columns_loc {
        if wildcard_tables.contains(tid) {
            cols.insert(*cid);
        }
    }
    cols
}

/// Applies adjustments to second part of a pipeline when it's split:
/// - prepend pipeline with From
/// - redefine columns materialized in preceding pipeline
/// - redirect all references to original columns to the new ones
pub fn anchor_split(
    context: &mut AnchorContext,
    first_table_name: &str,
    cols_at_split: &[CId],
    second_pipeline: Vec<Transform>,
) -> Vec<Transform> {
    let new_tid = context.ids.gen_tid();

    // define columns of the new CTE
    let mut columns_redirect = HashMap::<CId, CId>::new();
    let mut new_columns = Vec::new();
    for old_cid in cols_at_split {
        let new_cid = context.ids.gen_cid();
        columns_redirect.insert(*old_cid, new_cid);

        let old_def = context.columns_defs.get(old_cid).unwrap();

        let new_def = ColumnDef {
            id: new_cid,
            kind: match &old_def.kind {
                ColumnDefKind::Wildcard(tid) => ColumnDefKind::Wildcard(*tid),
                ColumnDefKind::ExternRef(name) => ColumnDefKind::ExternRef(name.clone()),
                ColumnDefKind::Expr { .. } => {
                    ColumnDefKind::ExternRef(context.ensure_column_name(old_cid))
                }
            },
        };
        context.columns_defs.insert(new_cid, new_def.clone());
        context.columns_loc.insert(new_cid, new_tid);
        new_columns.push(new_def);
    }

    // define a new local table
    context.table_defs.insert(
        new_tid,
        TableDef {
            name: first_table_name.to_string(),
            expr: TableExpr::Ref(
                TableRef::LocalTable(first_table_name.to_string()),
                new_columns.clone(),
            ),
            columns: new_columns,
        },
    );

    // split the pipeline
    let mut pipeline = second_pipeline;

    // adjust second part: prepend from and rewrite expressions to use new columns
    pipeline.insert(0, Transform::From(new_tid));

    let mut redirector = CidRedirector {
        redirects: columns_redirect,
    };
    redirector.fold_transforms(pipeline).unwrap()
}

/// Determines whether a pipeline must be split at a transform to
/// fit into one SELECT statement.
///
/// `following` contain names of following transforms in the pipeline.
fn is_split_required(transform: &Transform, following: &HashSet<String>) -> bool {
    // Pipeline must be split when there is a transform that is out of order:
    // - from (max 1x),
    // - join (no limit),
    // - filters (for WHERE)
    // - aggregate (max 1x)
    // - sort (no limit)
    // - filters (for HAVING)
    // - take (no limit)

    // Select and Compute are not affected by the order.

    match transform.as_ref() {
        "From" => following.contains("From"),
        "Join" => following.contains("From"),
        "Aggregate" => following.contains("Join") || following.contains("Aggregate"),
        "Sort" => following.contains("Join") || following.contains("Aggregate"),
        "Filter" => following.contains("Join"),

        // There can be many takes, but they have to be consecutive
        // For example `take 100 | sort a | take 10` can't be one CTE.
        // But this is enforced by other transforms anyway.
        "Take" => {
            following.contains("Join")
                || following.contains("Filter")
                || following.contains("Aggregate")
                || following.contains("Sort")
        }

        _ => false,
    }
}

/// An input requirement of a transform.
struct Requirement {
    pub col: CId,
    pub max_complexity: Complexity,
}

fn get_requirements(transform: &Transform) -> Vec<Requirement> {
    use Transform::*;

    let cids = match transform {
        Select(cids) | Aggregate(cids) => cids.clone(),
        Filter(expr) | Join { filter: expr, .. } => {
            CidCollector::collect(expr.clone()).into_iter().collect()
        }
        Sort(sorts) => sorts.iter().map(|s| s.column).collect(),
        Take(range) => {
            let mut cids = Vec::new();
            if let Some(e) = &range.start {
                cids.extend(CidCollector::collect(e.clone()));
            }
            if let Some(e) = &range.end {
                cids.extend(CidCollector::collect(e.clone()));
            }
            cids
        }

        From(_) | Compute(_) | Unique => return Vec::new(),
    };

    let max_complexity = match transform {
        Select(_) => Complexity::Windowed,
        Filter(_) => Complexity::Expr,

        // TODO: change this to Ident when aggregate is refactored to contain by instead of assigns
        Aggregate(_) => Complexity::Expr,

        Sort(_) => Complexity::Ident,
        Take(_) => Complexity::Expr,
        Join { .. } => Complexity::Expr,

        From(_) | Compute(_) | Unique => unreachable!(),
    };

    cids.into_iter()
        .map(|cid| Requirement {
            col: cid,
            max_complexity,
        })
        .collect()
}

/// Recursively inline column references that can materialize with
/// given complexity.
///
/// Returns column references that were not materialized.
fn anchor_column(
    context: &mut AnchorContext,
    req: Requirement,
    inputs_avail: &HashSet<CId>,
) -> HashSet<CId> {
    let col_def = &context.columns_defs[&req.col];

    match &col_def.kind {
        ColumnDefKind::Expr { expr, .. } => {
            let (mat, inputs) =
                Materializer::materialize(expr.clone(), req.max_complexity, inputs_avail, context);

            let col_def = context.columns_defs.get_mut(&req.col).unwrap();
            let (_, expr) = &mut col_def.kind.as_expr_mut().unwrap();
            **expr = mat;

            // if let ExprKind::ExternRef { .. } = &expr.kind {
            //     inputs.insert(req.col);
            // }

            inputs
        }
        ColumnDefKind::Wildcard(_) | ColumnDefKind::ExternRef(_) => HashSet::from([req.col]),
    }
}

struct Materializer<'a> {
    context: &'a mut AnchorContext,

    max_complexity: Complexity,
    inputs_avail: &'a HashSet<CId>,

    inputs_required: HashSet<CId>,
}

/// Complexity of a column expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Complexity {
    /// Only idents
    Ident,
    /// Everything except windowed expressions
    Expr,
    /// Everything including windowed expressions
    Windowed,
}

impl<'a> Materializer<'a> {
    fn materialize(
        expr: Expr,
        max_complexity: Complexity,
        inputs_avail: &HashSet<CId>,
        context: &'a mut AnchorContext,
    ) -> (Expr, HashSet<CId>) {
        let mut m = Materializer {
            context,
            max_complexity,
            inputs_avail,
            inputs_required: HashSet::new(),
        };

        let expr = m.fold_expr(expr).unwrap();

        (expr, m.inputs_required)
    }
}

impl<'a> IrFold for Materializer<'a> {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        if let ExprKind::ColumnRef(cid) = &expr.kind {
            // is this column available in one of input tables?
            if !self.inputs_avail.contains(cid) {
                // it is not, try to materialize it

                let def = &self.context.columns_defs[cid];
                let complexity = infer_complexity(def);

                if complexity > self.max_complexity {
                    // complexity too high, put off materialization
                } else {
                    // in-place materialization
                    return match &def.kind {
                        ColumnDefKind::Wildcard(_) => Ok(expr),
                        ColumnDefKind::ExternRef(_) => Ok(expr),
                        ColumnDefKind::Expr { expr, .. } => self.fold_expr(expr.clone()),
                    };
                }
            }
            self.inputs_required.insert(*cid);
        }

        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }

    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        // just a check that everything is folded as it should be
        assert!(self.inputs_avail.contains(&cid) || self.inputs_required.contains(&cid));
        Ok(cid)
    }
}

fn infer_complexity(col_def: &ColumnDef) -> Complexity {
    use crate::ir::ExprKind::*;
    use Complexity::*;

    match &col_def.kind {
        ColumnDefKind::Wildcard(_) => Ident,
        ColumnDefKind::Expr { expr, .. } => match &expr.kind {
            ColumnRef(_) => Ident,
            _ => Expr,
        },
        ColumnDefKind::ExternRef(_) => Ident,
    }
}

#[derive(Default)]
pub struct CidCollector {
    cids: HashSet<CId>,
}

impl CidCollector {
    pub fn collect(expr: Expr) -> HashSet<CId> {
        let mut collector = CidCollector::default();
        collector.fold_expr(expr).unwrap();
        collector.cids
    }
}

impl IrFold for CidCollector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        self.cids.insert(cid);
        Ok(cid)
    }
}

struct CidRedirector {
    redirects: HashMap<CId, CId>,
}

impl IrFold for CidRedirector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(self.redirects.get(&cid).cloned().unwrap_or(cid))
    }
}
