use super::ast::*;
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use sqlparser::ast::*;

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

pub fn to_select(pipeline: &Pipeline) -> Result<sqlparser::ast::Select> {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on `to_select` for that?

    // Alternatively we could do a single pass, but we need to split by before &
    // after the `aggregate`. If we did a single pass, do something like:
    // group_pairs from https://stackoverflow.com/a/65394297/3064736
    // let grouped = group_pairs(pipeline.iter().map(|t| (t.name, t)));
    // let from = grouped.get(&TransformationType::From).unwrap().first().unwrap().clone();

    let from = pipeline
        .iter()
        // .find(|t| matches!(t, Transformation::From(_)))
        .filter_map(|t| match t {
            Transformation::From(ident) => Some(sqlparser::ast::TableWithJoins {
                relation: sqlparser::ast::TableFactor::Table {
                    name: ObjectName(
                        ident
                            .iter()
                            .map(|i| i.clone().try_into().unwrap())
                            // .map(|i| TryInto::<sqlparser::ast::Ident>::try_into(i.clone()).unwrap())
                            .collect(),
                    ),
                    alias: None,
                    args: vec![],
                    with_hints: vec![],
                },
                joins: vec![],
            }),
            _ => None,
        })
        .collect();

    // We could combine the next two with a more sophisticated `split_at`

    // Find the filters that come before the aggregation.
    let where_ = match combine_filters(
        pipeline
            .iter()
            .take_while(|t| !matches!(t, Transformation::Aggregate { .. }))
            .filter(|t| matches!(t, Transformation::Filter(_)))
            .cloned()
            .collect(),
    ) {
        Transformation::Filter(items) => Item::Items(items).try_into()?,
        _ => unreachable!(),
    };

    // Find the filters that come after the aggregation.
    let having = match combine_filters(
        pipeline
            .iter()
            .skip_while(|t| !matches!(t, Transformation::Aggregate { .. }))
            .filter(|t| matches!(t, Transformation::Filter(_)))
            .cloned()
            .collect(),
    ) {
        Transformation::Filter(items) => Item::Items(items).try_into()?,
        _ => unreachable!(),
    };

    let take = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Take(_) => Some(t.clone().try_into()),
            _ => None,
        })
        .last()
        // TODO: change this into a result that returns an error if there's an
        // invalid take
        .map(|x| x.unwrap());

    // Find the final sort (none of the others affect the result, and can be discarded).
    let order_by = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Sort(items) => Some(
                items
                    .iter()
                    // TryInto::<sqlparser::ast::Expr>::try_into
                    .map(|i| i.clone().try_into())
                    .collect::<Result<Vec<_>>>(),
            ),
            _ => None,
        })
        .last()
        .unwrap_or(Ok(vec![]))?;

    // TODO: clean this rust up
    let aggregate = pipeline
        .iter()
        .find(|t| matches!(t, Transformation::Aggregate { .. }));
    let (group_bys, select_from_aggregate) = match aggregate {
        Some(Transformation::Aggregate { by, calcs }) => (
            (by.clone()),
            Some(
                calcs
                    .iter()
                    .map(|x| x.clone().try_into())
                    .collect::<Result<Vec<_>>>()?,
            ),
        ),
        None => (vec![], None),
        _ => unreachable!("Expected an aggregate transformation"),
    };
    let group_by = group_bys
        .iter()
        // TODO: Needs to be changed to treat these as a comma-ed list
        .map(|i| i.clone().try_into().unwrap())
        .collect();

    let select_from_derive = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Derive(assigns) => Some(assigns.clone()),
            _ => None,
        })
        .flatten()
        .map(|assign| assign.into())
        .collect::<Vec<SelectItem>>();

    // Only the final select matters (assuming we don't have notions of `select
    // *` or `select * except`)
    let select_from_select = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Select(items) => Some(
                items
                    .iter()
                    .map(|x| (x).clone().try_into())
                    .collect::<Result<Vec<_>>>(),
            ),
            _ => None,
        })
        .last()
        // TODO: handle result
        .map(|x| x.unwrap());

    let select = [
        select_from_select,
        select_from_aggregate,
        Some(select_from_derive),
    ]
    .into_iter()
    // TODO: should we do the option flattening here or in each of the selects?
    .flatten()
    .flatten()
    .collect();

    Ok(sqlparser::ast::Select {
        distinct: false,
        top: take,
        projection: select,
        from,
        group_by,

        // TODO: change these to be options above, rather than empty
        having: Some(having),
        selection: Some(where_),
        sort_by: order_by,
        lateral_views: vec![],
        distribute_by: vec![],
        cluster_by: vec![],
    })
}

// Alternatively this could be a `TryInto` impl?
// TODO: this should return a result.
/// Convert a pipeline into a number of pipelines which can each "fit" into a CTE.
pub fn queries_of_pipeline(pipeline: &Pipeline) -> Vec<Pipeline> {
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
    let mut queries = vec![];
    let mut counts: HashMap<&str, u32> = HashMap::new();

    let mut current_cte: Pipeline = vec![];

    // This seems inelegant! I'm sure there's a better way to do this, though
    // note the constraints from above.
    for transformation in pipeline {
        if transformation.name() == "aggregate" && counts.get("aggregate") == Some(&1) {
            // We have a new CTE
            queries.push(current_cte);
            current_cte = vec![];
            counts.clear();
        }

        *counts.entry(transformation.name()).or_insert(0) += 1;
        current_cte.push(transformation.to_owned());

        if counts.get("take") == Some(&1) {
            // We have a new CTE
            queries.push(current_cte);
            current_cte = vec![];
            counts.clear();
        }
    }
    if !current_cte.is_empty() {
        queries.push(current_cte);
    }

    queries
}

// TODO: change to TryInto.
impl From<Assign> for SelectItem {
    fn from(assign: Assign) -> Self {
        SelectItem::ExprWithAlias {
            alias: sqlparser::ast::Ident {
                value: assign.lvalue,
                quote_style: None,
            },
            expr: Item::Items(assign.rvalue).try_into().unwrap(),
        }
    }
}

// Hack because of orphan rules
impl TryFrom<Item> for sqlparser::ast::SelectItem {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sqlparser::ast::SelectItem::UnnamedExpr(
                sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new(ident)),
            )),
            Item::List(items) | Item::Items(items) => Ok(sqlparser::ast::SelectItem::UnnamedExpr(
                sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new(
                    // TODO: temp hack
                    TryInto::<sqlparser::ast::Expr>::try_into(Item::Items(items))
                        .unwrap()
                        .to_string(),
                )),
            )),

            _ => Err(anyhow!(
                "Can't convert to SelectItem at the moment; {:?}",
                item
            )),
        }
    }
}

pub trait ToSql {
    fn to_sql(&self) -> sqlparser::ast::Expr;
}
// Tried this but I don't think it works:
// impl From<&dyn ToSql> for sqlparser::ast::Expr {
//     fn from(to_sql: &dyn ToSql) -> sqlparser::ast::Expr {
//         to_sql.to_sql()
//     }
// }
impl ToSql for Items {
    fn to_sql(&self) -> sqlparser::ast::Expr {
        sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new(
            self.iter()
                // FIXME
                .map(|item| item.as_ident().unwrap())
                .cloned()
                .collect::<Vec<String>>()
                .join(" "),
        ))
    }
}

impl TryFrom<Transformation> for sqlparser::ast::Top {
    type Error = anyhow::Error;
    fn try_from(transformation: Transformation) -> Result<Self> {
        match transformation {
            Transformation::Take(take) => Ok(sqlparser::ast::Top {
                // TODO: implement for number
                quantity: Some(Item::Raw(take.to_string()).try_into()?),
                with_ties: false,
                percent: false,
            }),
            _ => Err(anyhow!("Top transformation only supported for Take")),
        }
    }
}

impl TryFrom<Item> for sqlparser::ast::Expr {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sqlparser::ast::Expr::Identifier(
                sqlparser::ast::Ident::new(ident),
            )),
            Item::Raw(ident) => Ok(sqlparser::ast::Expr::Identifier(
                sqlparser::ast::Ident::new(ident),
            )),
            // TODO: List needs a different impl
            Item::Items(items) | Item::List(items) => Ok(sqlparser::ast::Expr::Identifier(
                sqlparser::ast::Ident::new(
                    items
                        .iter()
                        .map(|item| {
                            TryInto::<sqlparser::ast::Expr>::try_into(item.clone()).unwrap()
                        })
                        // .cloned()
                        .collect::<Vec<sqlparser::ast::Expr>>()
                        .iter()
                        .map(|x| x.to_string())
                        .join(" "),
                ),
            )),
            Item::String(ident) => Ok(sqlparser::ast::Expr::Value(
                sqlparser::ast::Value::DoubleQuotedString(ident),
            )),
            _ => Err(anyhow!("Can't convert to Expr at the moment; {:?}", item)),
        }
    }
}

impl TryFrom<Item> for sqlparser::ast::Ident {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sqlparser::ast::Ident::new(ident)),
            Item::Raw(ident) => Ok(sqlparser::ast::Ident::new(ident)),
            _ => Err(anyhow!("Can't convert to Ident at the moment; {:?}", item)),
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use insta::assert_display_snapshot;
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
        let queries = queries_of_pipeline(&pipeline);
        assert_eq!(queries.len(), 1);

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
        let queries = queries_of_pipeline(&pipeline);
        assert_eq!(queries.len(), 2);

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
        let queries = queries_of_pipeline(&pipeline);
        assert_eq!(queries.len(), 3);
    }

    #[test]
    fn test_to_select() {
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
        let cte = to_select(&pipeline).unwrap();
        // TODO: totally wrong but compiles, and we're on our way to fixing it.
        assert_display_snapshot!(cte, @r###"SELECT TOP (20) average salary FROM employees WHERE country = ""USA"" GROUP BY title country SORT BY title HAVING "###);
    }
}
