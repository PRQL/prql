use std::collections::HashMap;

use super::parser::{Pipeline, TransformationType};

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
    let mut ctes = vec![];
    let mut counts: HashMap<&TransformationType, u32> = HashMap::new();

    let mut current_cte: Pipeline = vec![];

    // This seems inelegant! I'm sure there's a better way to do this.
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
