use anyhow::Result;
use core::panic;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

use crate::ast::TableExternRef;
use crate::ir::{
    fold_transform, CId, ColumnDef, ColumnDefKind, Expr, ExprKind, IrFold, TableDef, TableExpr,
    TableRef, Transform,
};

use super::context::AnchorContext;

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

    log::debug!("traversing pipeline to obtain columns: {output_cols:?}");

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
            let r_inputs = anchor_column(context, r.col, r.max_complexity, &inputs_avail);

            output_cols.extend(&r_inputs - &inputs_avail);
            inputs_required.extend(r_inputs);
        }

        // push into current pipeline
        if !matches!(transform, Transform::Select(_)) {
            curr_pipeline_rev.push(transform);
        }
    }

    // prevent finishing if there are still missing requirements
    let has_all_inputs = inputs_required.iter().all(|c| inputs_avail.contains(c));
    if !has_all_inputs && pipeline.is_empty() {
        // push From back to the remaining pipeline
        let transform = curr_pipeline_rev.pop().unwrap();
        if let Transform::From(_) = &transform {
            pipeline.push(transform);
        } else {
            panic!("pipeline does not start with From!");
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
                .map(|tiid| context.register_column(ColumnDefKind::Wildcard, *tiid))
                .collect()
        } else {
            cols
        };

        curr_pipeline_rev.push(Transform::Select(cols));
    }

    let remaining_pipeline = if pipeline.is_empty() {
        None
    } else {
        log::debug!("splitting avail={inputs_avail:?} required={inputs_required:?}");

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
        .filter_map(|cid| match context.columns_defs[cid].kind {
            ColumnDefKind::Wildcard => Some(context.columns_loc[cid]),
            _ => None,
        })
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
    ctx: &mut AnchorContext,
    first_table_name: &str,
    cols_at_split: &[CId],
    second_pipeline: Vec<Transform>,
) -> Vec<Transform> {
    let new_tid = ctx.tid.gen();

    // define columns of the new CTE
    let mut cid_redirects = HashMap::<CId, CId>::new();
    let mut new_columns = Vec::new();
    for old_cid in cols_at_split {
        let old_def = ctx.columns_defs.get(old_cid).unwrap();

        let kind = match &old_def.kind {
            ColumnDefKind::Wildcard => ColumnDefKind::Wildcard,
            ColumnDefKind::ExternRef(name) => ColumnDefKind::ExternRef(name.clone()),
            ColumnDefKind::Expr { .. } => ColumnDefKind::ExternRef(ctx.ensure_column_name(old_cid)),
        };

        let id = ctx.cid.gen();
        let window = None;
        let col = ColumnDef { id, kind, window };

        new_columns.push(col);
        cid_redirects.insert(*old_cid, id);
    }

    // define a new table
    ctx.table_defs.insert(
        new_tid,
        TableDef {
            id: new_tid,
            name: Some(first_table_name.to_string()),
            // here we should put the pipeline, but because how this function is called,
            // we need to return the pipeline directly, so we just instert dummy expr instead
            expr: TableExpr::ExternRef(TableExternRef::LocalTable("".to_string()), vec![]),
        },
    );

    // define instance of that table
    let table_ref = TableRef {
        source: new_tid,
        name: None,
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
            let def = ctx.columns_defs.get_mut(&cid).unwrap();
            def.kind = extern_ref;
        } else {
            panic!("cannot infer an name for {cid:?}")
        }
    }
}

fn infer_extern_ref(cid: CId, ctx: &AnchorContext) -> Option<ColumnDefKind> {
    let def = &ctx.columns_defs[&cid];

    match &def.kind {
        ColumnDefKind::Wildcard | ColumnDefKind::ExternRef(_) => Some(def.kind.clone()),
        ColumnDefKind::Expr { name, expr } => {
            if let Some(name) = name {
                Some(ColumnDefKind::ExternRef(name.clone()))
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
        Select(cids) | Aggregate { by: cids } => cids.clone(),
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

        Aggregate { .. } => Complexity::Ident,

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
    cid: CId,
    max_complexity: Complexity,
    inputs_avail: &HashSet<CId>,
) -> HashSet<CId> {
    let col_def = &context.columns_defs[&cid];

    match &col_def.kind {
        ColumnDefKind::Expr { expr, .. } => {
            let (mat, inputs) =
                Materializer::materialize(expr.clone(), max_complexity, inputs_avail, context);

            let col_def = context.columns_defs.get_mut(&cid).unwrap();
            let (_, expr) = &mut col_def.kind.as_expr_mut().unwrap();
            **expr = mat;

            inputs
        }
        ColumnDefKind::Wildcard | ColumnDefKind::ExternRef(_) => {
            if inputs_avail.contains(&cid) {
                HashSet::from([cid])
            } else {
                panic!("cannot anchor {:?}. This is probably caused by bad IR", cid)
            }
        }
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
            let def = &self.context.columns_defs[cid];
            match &def.kind {
                ColumnDefKind::Expr { expr, .. } => {
                    let complexity = infer_complexity(def);

                    if complexity > self.max_complexity {
                        // complexity too high, put off materialization
                    } else {
                        // in-place materialization
                        return self.fold_expr(expr.clone());
                    }
                }

                // no need to materialize
                ColumnDefKind::Wildcard | ColumnDefKind::ExternRef(_) => {}
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
        ColumnDefKind::Expr { expr, .. } => {
            if col_def.window.is_some() {
                Windowed
            } else {
                match &expr.kind {
                    ColumnRef(_) => Ident,
                    _ => Expr,
                }
            }
        }
        ColumnDefKind::Wildcard => Ident,
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
                let cd = self.fold_column_def(cd)?;
                self.ctx.columns_defs.insert(cd.id, cd.clone());
                Ok(Transform::Compute(cd))
            }
            _ => fold_transform(self, transform),
        }
    }
}
