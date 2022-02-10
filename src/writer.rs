use std::collections::HashMap;

use super::parser::{Pipeline, Transformation, TransformationType};

#[allow(dead_code)]
fn to_ctes<'a>(pipeline: &Pipeline<'a>) -> Vec<Pipeline<'a>> {
    // Before starting a new CTE, we can have a pipeline with:
    // - 1 aggregate.
    // - 1 take, and then 0 other transformations.
    // - (I think filters can be combined. After combining them we can
    //   have 1 filter before the aggregate (`WHERE`) and 1 filter after the
    //   aggregate (`HAVING`).)
    //
    // So we loop through the Pipeline, and cut it into cte-sized pipelines,
    // which we'll then compose together.
    let mut ctes = vec![];
    let mut counts: HashMap<TransformationType, u32> = HashMap::new();

    let mut current_cte: Vec<Transformation> = vec![];
    for transformation in pipeline {
        *counts.entry(transformation.name.clone()).or_insert(0) += 1;

        if let TransformationType::Aggregate = transformation.name {
            if counts.get(&TransformationType::Aggregate) == Some(&1) {
                // We have a new CTE
                ctes.push(current_cte);
                current_cte = vec![];
            }
        }

        current_cte.push(transformation.clone());

        if counts.get(&TransformationType::Take) == Some(&1) {
            // We have a new CTE
            ctes.push(current_cte);
            current_cte = vec![];
        }
    }

    ctes
}
