use std::collections::HashMap;

use crate::{
    ir::rq::{CId, Transform},
    sql::{pq::context::RIId, pq_ast::SqlTransform},
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
    }

    /// Reorder or remove columns to make `Union` happy.
    pub(crate) fn apply_active_mapping(&mut self, output: Vec<CId>) -> Vec<CId> {
        if let Some(mapping) = &self.active_positional_mapping {
            let new_output = mapping.iter().map(|idx| output[*idx]).collect();
            log::debug!("remapping {output:?} to {new_output:?}");
            new_output
        } else {
            output
        }
    }

    pub fn compute_and_store_mapping(
        &mut self,
        (before_riid, before): &(RIId, Vec<CId>),
        (after_riid, after): &(RIId, Vec<CId>),
    ) {
        let riid = if after_riid != before_riid {
            log::warn!(
                ".. comparing positions for different relations {before_riid:?} vs {after_riid:?} "
            );
            return;
        } else {
            after_riid
        };

        if after == before {
            log::trace!(".. relation {after_riid:?} is already correctly mapped: {after:?}");
            return;
        }

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

        match self.relation_positional_mapping.get(riid) {
            Some(already_recorded) => {
                if already_recorded != &mapping {
                    log::warn!("relation {riid:?} appear twice with different mapping {already_recorded:?} vs {mapping:?}");
                }
            }
            None => {
                log::debug!(".. relation {riid:?} will be mapped: {mapping:?}");
                self.relation_positional_mapping.insert(*riid, mapping);
            }
        }
    }
}

/// Outputs the columns required for position sensitive transforms in the pipeline.
pub fn compute_positional_mappings(
    pipeline: &[SqlTransform<RIId, Transform>],
) -> Vec<(RIId, Vec<CId>)> {
    let mut constraints = vec![];
    let mut columns = vec![];

    log::trace!("traversing pipeline to obtain positional mapping:");

    for transform in pipeline {
        match transform {
            SqlTransform::Super(s) => match s {
                Transform::Compute(compute) => {
                    if !columns.contains(&compute.id) {
                        columns.push(compute.id);
                    }
                }
                Transform::Select(cids) => {
                    columns.clear();
                    columns.extend_from_slice(cids.as_slice());
                }
                Transform::Aggregate { partition, compute } => {
                    columns.clear();
                    columns.extend_from_slice(partition.as_slice());
                    columns.extend_from_slice(compute.as_slice());
                }
                t @ Transform::Append(_) | t @ Transform::From(_) => {
                    log::warn!("compute_positional_mapping_before: {t:?} is not implemented");
                }
                _ => (),
            },
            SqlTransform::Select(cids) => {
                columns.clear();
                columns.extend_from_slice(cids.as_slice());
            }
            SqlTransform::Aggregate { partition, compute } => {
                columns.clear();
                columns.extend_from_slice(partition.as_slice());
                columns.extend_from_slice(compute.as_slice());
            }
            SqlTransform::Except { bottom, .. }
            | SqlTransform::Intersect { bottom, .. }
            | SqlTransform::Union { bottom, .. } => {
                constraints.push((*bottom, columns.clone()));
            }
            t @ SqlTransform::DistinctOn(_) => {
                log::warn!("compute_positional_mapping_before: {t:?} is not implemented");
            }
            _ => (),
        }
        log::trace!(".. columns after {}: {columns:?}", transform.as_str());
    }

    constraints
}
