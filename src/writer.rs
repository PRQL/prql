use super::ast::*;
use std::collections::HashMap;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Cte {
    // TODO: Refine this to more concrete types as we build them out, or use
    // rs-sqlparser https://github.com/max-sixty/prql/issues/97?
    //
    // Should they be Option<T>? Or just empty if they're not required?
    select: Option<Items>,
    from: Transformation,
    where_: Option<Transformation>,
    group_by: Option<Items>,
    having: Option<Transformation>,
    order_by: Option<Transformation>,
}

/// Combines filters by putting them in parentheses and then joining them with `and`.
/// Note that this is very hacky and probably `Filter` should be a type which
/// this is implemented on.
#[allow(unstable_name_collisions)] // Same behavior as the std lib; we can remove this + itertools when that's released.
fn combine_filters(filters: Vec<Transformation>) -> Transformation {
    Transformation::Filter(
        filters
            .into_iter()
            .map(|filter| match filter {
                Transformation::Filter(items) => Item::Items(items),
                _ => {
                    panic!("Can only combine filters with other filters.");
                }
            })
            .intersperse(Item::Raw("and".to_owned()))
            .collect(),
    )
}

pub fn to_cte(pipeline: &Pipeline) -> Cte {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on `to_ctes` for that?

    // Alternatively we could do a single pass, but we need to split by before &
    // after the `aggregate`. If we did a single pass, do something like:
    // group_pairs from https://stackoverflow.com/a/65394297/3064736
    // let grouped = group_pairs(pipeline.iter().map(|t| (t.name, t)));
    // let from = grouped.get(&TransformationType::From).unwrap().first().unwrap().clone();

    let from = pipeline
        .iter()
        .find(|t| matches!(t, Transformation::From(_)))
        .unwrap()
        .clone();

    // We could combine the next two with a more sophisticated `split_at`

    // Find the filters that come before the aggregation.
    let where_ = combine_filters(
        pipeline
            .iter()
            .take_while(|t| !matches!(t, Transformation::Aggregate { by: _, calcs: _ }))
            .filter(|t| matches!(t, Transformation::Filter(_)))
            .cloned()
            .collect(),
    );

    // Find the filters that come after the aggregation.
    let having = combine_filters(
        pipeline
            .iter()
            .skip_while(|t| !matches!(t, Transformation::Aggregate { by: _, calcs: _ }))
            .filter(|t| matches!(t, Transformation::Filter(_)))
            .cloned()
            .collect(),
    );

    // Find the final sort (none of the others affect the result, and can be discarded).
    let order_by = pipeline
        .iter()
        .filter(|t| matches!(t, Transformation::Sort(_)))
        .cloned()
        .last();

    // TODO: clean this rust up
    let aggregate = pipeline
        .iter()
        .find(|t| matches!(t, Transformation::Aggregate { by: _, calcs: _ }));
    let (group_by, select_from_aggregate) = match aggregate {
        Some(Transformation::Aggregate { by, calcs }) => (Some(by.clone()), Some(calcs.clone())),
        None => (None, None),
        _ => unreachable!("Expected an aggregate transformation"),
    };

    // Only the final select matters (assuming we don't have notions of `select
    // *` or `select * except`)
    let select_from_select = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Select(items) => Some(items.clone()),
            _ => None,
        })
        .last();

    // TODO: add selects from derives
    let select = [select_from_select, select_from_aggregate]
        .into_iter()
        // This unwraps the Options
        .flatten()
        // This flattens the vecs
        .flatten()
        .collect();

    Cte {
        select: Some(select),
        from,
        order_by,
        group_by,
        having: Some(having),
        where_: Some(where_),
    }
}

/// Convert a pipeline into a number of pipelines which can each "fit" into a CTE.
pub fn to_ctes(pipeline: &Pipeline) -> Vec<Pipeline> {
    // Before starting a new CTE, we can have a pipeline with:
    // - 1 aggregate.
    // - 1 take, and then 0 other transformations.
    // - (I think filters can be combined. After combining them we can
    //   have 1 filter before the aggregate (`WHERE`) and 1 filter after the
    //   aggregate (`HAVING`).)
    //
    // So we loop through the Pipeline, and cut it into cte-sized pipelines,
    // which we'll then compose together.

    // TODO: how do we handle the `from` of the next query? Add it here? Have a
    // Vec<Ctc> where this is implicit?
    let mut ctes = vec![];
    let mut counts: HashMap<&str, u32> = HashMap::new();

    let mut current_cte: Pipeline = vec![];

    // This seems inelegant! I'm sure there's a better way to do this, though
    // note the constraints from above.
    for transformation in pipeline {
        if transformation.name() == "aggregate" && counts.get("aggregate") == Some(&1) {
            // We have a new CTE
            ctes.push(current_cte);
            current_cte = vec![];
            counts.clear();
        }

        *counts.entry(transformation.name()).or_insert(0) += 1;
        current_cte.push(transformation.to_owned());

        if counts.get("take") == Some(&1) {
            // We have a new CTE
            ctes.push(current_cte);
            current_cte = vec![];
            counts.clear();
        }
    }
    if !current_cte.is_empty() {
        ctes.push(current_cte);
    }

    ctes
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::assert_yaml_snapshot;
    use serde_yaml::from_str;

    use crate::ast::Pipeline;

    #[test]
    fn test_to_ctes() {
        // One aggregate, take at the end
        let yaml: &str = r###"
- From:
    - Ident: employees
- Filter:
    - Ident: country
    - Raw: "="
    - String: "\"USA\""
- Aggregate:
    by:
      - List:
          - Ident: title
          - Ident: country
    calcs:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
- Sort:
    - Ident: title
- Take: 20
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 1);

        // One aggregate, but take at the top
        let yaml: &str = r###"
- From:
    - Ident: employees
- Take: 20
- Filter:
    - Ident: country
    - Raw: "="
    - String: "\"USA\""
- Aggregate:
    by:
      - List:
          - Ident: title
          - Ident: country
    calcs:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
- Sort:
    - Ident: title
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 2);

        // A take, then two aggregates
        let yaml: &str = r###"
- From:
    - Ident: employees
- Take: 20
- Filter:
    - Ident: country
    - Raw: "="
    - String: "\"USA\""
- Aggregate:
    by:
      - List:
          - Ident: title
          - Ident: country
    calcs:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
- Aggregate:
    by:
      - List: []
    calcs:
      - List:
          - Items:
              - Ident: sum
              # TODO: this isn't currently defined
              - Ident: average_salary
- Sort:
    - Ident: sum_gross_cost
 
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 3);
    }

    #[test]
    fn test_to_cte() {
        let yaml: &str = r###"
- From:
    - Ident: employees
- Filter:
    - Ident: country
    - Raw: "="
    - String: "\"USA\""
- Aggregate:
    by:
      - List:
          - Ident: title
          - Ident: country
    calcs:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
- Sort:
    - Ident: title
- Take: 20
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let cte = to_cte(&pipeline);
        assert_yaml_snapshot!(cte, @r###"
        ---
        select:
          - List:
              - Items:
                  - Ident: average
                  - Ident: salary
        from:
          From:
            - Ident: employees
        where_:
          Filter:
            - Items:
                - Ident: country
                - Raw: "="
                - String: "\"USA\""
        group_by:
          - List:
              - Ident: title
              - Ident: country
        having:
          Filter: []
        order_by:
          Sort:
            - Ident: title
        "###);
    }
}
