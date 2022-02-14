use std::collections::HashMap;

use super::parser::{Items, Pipeline, Transformation, TransformationType};

#[allow(dead_code)]
pub struct Cte {
    // TODO: Refine this to more concrete types as we build them out.
    // Should they be Option<T>? Or just empty if they're not required?
    select: Option<Items>,
    from: Option<Items>,
    where_: Option<Items>,
    group_by: Option<Items>,
    having: Option<Items>,
    order_by: Option<Transformation>,
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
        .find(|t| t.name == TransformationType::From)
        .map(|t| t.args.clone());
    // We could combine the next two with a more sophisticated `split_at`

    // Find the filters that come before the aggregation.
    // TODO: combine the filters.
    let where_ = pipeline
        .iter()
        .take_while(|t| t.name != TransformationType::Aggregate)
        .filter(|t| t.name == TransformationType::Filter)
        // FIXME: this is just concatting everything; we need to add ANDs (and
        // probably do it properly)
        .flat_map(|t| t.args.clone())
        .collect();

    // Find the filters that come after the aggregation.
    // TODO: combine the filters.
    let having = pipeline
        .iter()
        .skip_while(|t| t.name != TransformationType::Aggregate)
        .filter(|t| t.name == TransformationType::Filter)
        // FIXME: this is just concatting everything; we need to add ANDs (and
        // probably do it well)
        .flat_map(|t| t.args.clone())
        .collect();

    // Find the final sort (none of the others affect the result, and can be discarded).
    let order_by = pipeline
        .iter()
        .filter(|t| t.name == TransformationType::Sort)
        .cloned()
        .last();

    let selects = pipeline
        .iter()
        .find(|t| t.name == TransformationType::Aggregate);

    let select_from_aggregate = selects.map(|aggregate| aggregate.args.clone());
    let group_by = selects
        .and_then(|aggregate| aggregate.named_args.first())
        .map(|named_arg| {
            assert!(named_arg.lvalue == "by");
            named_arg.rvalue.clone()
        });

    // Only the final select matters (assuming we don't have notions of `select
    // *` or `select * except`)
    let select_from_select = pipeline
        .iter()
        .filter(|t| t.name == TransformationType::Select)
        .last()
        .map(|t| t.args.clone());

    // Code smell that we're using the PRQL AST to store SQL, and so giving
    // empty named args etc.
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
    let mut counts: HashMap<&TransformationType, u32> = HashMap::new();

    let mut current_cte: Pipeline = vec![];

    // This seems inelegant! I'm sure there's a better way to do this, though
    // note the constraints from above.
    for transformation in pipeline {
        if let TransformationType::Aggregate = transformation.name {
            if counts.get(&TransformationType::Aggregate) == Some(&1) {
                // We have a new CTE
                ctes.push(current_cte);
                current_cte = vec![];
                counts.clear();
            }
        }

        *counts.entry(&transformation.name).or_insert(0) += 1;
        current_cte.push(transformation.to_owned());

        if counts.get(&TransformationType::Take) == Some(&1) {
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
    use serde_yaml::from_str;

    use crate::parser::Pipeline;

    #[test]
    fn test_to_ctes() {
        // One aggregate, take at the end
        let yaml: &str = r###"
  - name: From
    args:
      - Ident: employees
    named_args: []
  - name: Filter
    args:
      - Ident: country
      - Raw: "="
      - String: "\"USA\""
    named_args: []
  - name: Aggregate
    args:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
    named_args: []
  - name: Sort
    args:
      - Ident: sum_gross_cost
    named_args: []
  - name: Take
    args:
      - Raw: "20"
    named_args: []
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 1);

        // One aggregate, but take at the top
        let yaml: &str = r###"
  - name: From
    args:
      - Ident: employees
    named_args: []
  - name: Take
    args:
      - Raw: "20"
    named_args: []
  - name: Filter
    args:
      - Ident: country
      - Raw: "="
      - String: "\"USA\""
    named_args: []
  - name: Aggregate
    args:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
    named_args: []
  - name: Sort
    args:
      - Ident: sum_gross_cost
    named_args: []
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 2);

        // A take, then two aggregates
        let yaml: &str = r###"
  - name: From
    args:
      - Ident: employees
    named_args: []
  - name: Take
    args:
      - Raw: "20"
    named_args: []
  - name: Filter
    args:
      - Ident: country
      - Raw: "="
      - String: "\"USA\""
    named_args: []
  - name: Aggregate
    args:
      - List:
          - Items:
              - Ident: average
              - Ident: salary
    named_args:
      - lvalue: by
        rvalue:
          - List:
              - Ident: title
              - Ident: country
  - name: Aggregate
    args:
      - List:
          - Items:
              - Ident: sum
              - Ident: average_salary
    named_args: []
        "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let ctes = to_ctes(&pipeline);
        assert_eq!(ctes.len(), 3);
    }
}
