use std::collections::HashMap;

use crate::{
    ir::rq::{CId, Compute, Transform},
    sql::{
        pq::{anchor::Requirements, context::RIId},
        pq_ast::SqlTransform,
    },
};

/// State required to properly handle the transforms that are order sensitive like `Union`.
#[derive(Default, Debug)]
pub struct PositionalMapper {
    pub relation_positional_mapping: HashMap<RIId, Vec<usize>>,
    pub active_positional_mapping: Option<Vec<usize>>,
}

impl PositionalMapper {
    /// Remember the mapping for this `RIId` to know what to apply for `apply_positional_mapping`.
    pub(crate) fn activate_mapping(&mut self, riid: &RIId) {
        self.active_positional_mapping = self.relation_positional_mapping.remove(riid);
        log::trace!(
            "loading remapping for {riid:?}: {:?}",
            self.active_positional_mapping
        );
    }

    /// Reorder or remove columns to make `Union` happy.
    pub(crate) fn apply_active_mapping(&mut self, output: Vec<CId>) -> Vec<CId> {
        if let Some(mapping) = &self.active_positional_mapping {
            // Check if the mapping indices are valid for the output
            if mapping.iter().any(|idx| *idx >= output.len()) {
                log::warn!(
                    "positional mapping indices out of bounds: mapping={mapping:?}, output_len={}",
                    output.len()
                );
                // If mapping is invalid, don't apply it
                return output;
            }

            let new_output = mapping.iter().map(|idx| output[*idx]).collect();
            log::debug!("remapping {output:?} to {new_output:?} via {mapping:?}");
            new_output
        } else {
            output
        }
    }

    pub fn compute_and_store_mapping(&mut self, before: &[CId], after: &[CId], riid: &RIId) {
        let mapping: Vec<_> = after
            .iter()
            .flat_map(|a| match before.iter().position(|b| b == a) {
                Some(idx) => Some(idx),
                None => {
                    log::warn!(".. no counterpart for column {a:?}");
                    None
                }
            })
            .collect();

        // Only store the mapping if it's complete (all columns matched)
        // If mapping is incomplete, it means the bottom relation hasn't been fully
        // compiled yet, so we shouldn't apply any mapping
        if mapping.len() == after.len() && !self.relation_positional_mapping.contains_key(riid) {
            log::debug!(".. relation {riid:?} will be mapped: {mapping:?}");
            self.relation_positional_mapping.insert(*riid, mapping);
        } else if mapping.len() != after.len() {
            log::debug!(
                ".. skipping incomplete mapping for {riid:?}: {}/{} columns matched",
                mapping.len(),
                after.len()
            );
        }
    }
}

/// Outputs the columns required for position sensitive transforms in the pipeline.
pub fn compute_positional_mappings(
    pipeline: &[SqlTransform<RIId, Transform>],
    requirements: Option<&Requirements>,
) -> Vec<(RIId, Vec<CId>)> {
    let mut constraints = vec![];
    let mut columns = vec![];

    log::trace!("traversing pipeline to obtain positional mapping:");

    // Only process selected columns to avoid surnumerary one
    let add_columns = |columns: &mut Vec<CId>, cids: &[CId]| {
        if let Some(requirements) = requirements {
            columns.extend(cids.iter().filter(|cid| requirements.is_selected(cid)));
        } else {
            columns.extend_from_slice(cids);
        }
    };

    for transform in pipeline {
        match transform {
            SqlTransform::Super(s) => match s {
                Transform::Compute(Compute { id, .. }) => {
                    if !columns.contains(id) {
                        add_columns(&mut columns, &[*id]);
                    }
                }
                Transform::Select(cids) => {
                    columns.clear();
                    add_columns(&mut columns, cids);
                }
                Transform::Aggregate { compute, .. } => {
                    columns.clear();
                    add_columns(&mut columns, compute);
                }
                _ => (),
            },
            SqlTransform::Except { bottom, .. }
            | SqlTransform::Intersect { bottom, .. }
            | SqlTransform::Union { bottom, .. } => {
                constraints.push((*bottom, columns.clone()));
                log::trace!(
                    ".. mapping for {}/{bottom:?}: {columns:?}",
                    transform.as_str()
                );
            }
            _ => (),
        }
        log::trace!(
            ".. selected columns after {}: {columns:?}",
            transform.as_str()
        );
    }

    constraints
}
