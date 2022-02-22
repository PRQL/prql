use super::ast::*;
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use sqlparser::ast::*;

pub fn to_select(pipeline: &Pipeline) -> Result<sqlparser::ast::Select> {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on `to_select` for that?
    // TODO: this doesn't handle joins at all yet.

    // Alternatively we could do a single pass, but we need to split by before &
    // after the `aggregate`, even before considering joins. If we did a single
    // pass, do something like: group_pairs from
    // https://stackoverflow.com/a/65394297/3064736
    // let grouped = group_pairs(pipeline.iter().map(|t| (t.name, t)));
    // let from = grouped.get(&TransformationType::From).unwrap().first().unwrap().clone();

    let from = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::From(ident) => Some(sqlparser::ast::TableWithJoins {
                relation: sqlparser::ast::TableFactor::Table {
                    name: ObjectName(
                        ident
                            .iter()
                            .map(|i| i.clone().try_into().unwrap())
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

    // Split the pipeline into before & after the aggregate
    let (before, after) = pipeline.split_at(
        pipeline
            .iter()
            .position(|t| matches!(t, Transformation::Aggregate { .. }))
            .unwrap_or(pipeline.len()),
    );
    // Convert the filters in a pipeline into an Expr
    fn filter_of_pipeline(pipeline: &[Transformation]) -> Result<Option<sqlparser::ast::Expr>> {
        let filters: Vec<Filter> = pipeline
            .iter()
            .take_while(|t| !matches!(t, Transformation::Aggregate { .. }))
            .filter_map(|t| match t {
                Transformation::Filter(filter) => Some(filter),
                _ => None,
            })
            .cloned()
            .collect();

        Ok(if !filters.is_empty() {
            Some((Item::Items(Filter::combine_filters(filters).0)).try_into()?)
        } else {
            None
        })
    }
    // Find the filters that come before the aggregation.
    let where_ = filter_of_pipeline(before).unwrap();
    let having = filter_of_pipeline(after).unwrap();

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
            Transformation::Sort(items) => {
                Some(items.iter().map(|i| i.clone().try_into()).try_collect())
            }
            _ => None,
        })
        .last()
        .unwrap_or(Ok(vec![]))?;

    // TODO: clean this rust up
    let aggregate = pipeline
        .iter()
        .find(|t| matches!(t, Transformation::Aggregate { .. }));
    let (group_bys, select_from_aggregate): (Vec<Item>, Option<Vec<SelectItem>>) = match aggregate {
        Some(Transformation::Aggregate { by, calcs, assigns }) => (
            by.clone(),
            // This is inscrutable, sorry for the rust.
            // It's chaining a) the calcs (such as `sum salary`) and b) the assigns
            // (such as `sum_salary: sum salary`), and converting them into
            // SelectItems.
            Some(
                calcs
                    .iter()
                    .map(|x| x.clone().try_into())
                    .chain(assigns.iter().map(|x| x.clone().try_into()))
                    .try_collect()?,
            ),
        ),
        None => (vec![], None),
        _ => unreachable!("Expected an aggregate transformation"),
    };
    let group_by = Item::List(group_bys).try_into()?;

    let select_from_derive = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Derive(assigns) => Some(assigns.clone()),
            _ => None,
        })
        .flatten()
        .map(|assign| assign.try_into())
        .try_collect()?;

    // Only the final select matters (assuming we don't have notions of `select
    // *` or `select * except`)
    let select_from_select = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Select(items) => {
                Some(items.iter().map(|x| (x).clone().try_into()).try_collect())
            }
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
        having,
        selection: where_,
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

impl TryFrom<Assign> for SelectItem {
    type Error = anyhow::Error;
    fn try_from(assign: Assign) -> Result<Self> {
        Ok(SelectItem::ExprWithAlias {
            alias: sqlparser::ast::Ident {
                value: assign.lvalue,
                quote_style: None,
            },
            expr: (*assign.rvalue).try_into()?,
        })
    }
}

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
impl TryFrom<Transformation> for sqlparser::ast::SelectItem {
    type Error = anyhow::Error;
    fn try_from(transformation: Transformation) -> Result<Self> {
        Ok(sqlparser::ast::SelectItem::UnnamedExpr(
            sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new(format!(
                "TODO: {:?}",
                &transformation
            ))),
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
            Item::Items(items) => Ok(sqlparser::ast::Expr::Identifier(
                sqlparser::ast::Ident::new(
                    items
                        .iter()
                        .map(|item| {
                            TryInto::<sqlparser::ast::Expr>::try_into(item.clone()).unwrap()
                        })
                        .collect::<Vec<sqlparser::ast::Expr>>()
                        .iter()
                        .map(|x| x.to_string())
                        // Currently a big hack, but maybe OK, since we don't
                        // need to parse every single expression into sqlparser ast.
                        .join(" "),
                ),
            )),
            Item::String(ident) => Ok(sqlparser::ast::Expr::Value(
                sqlparser::ast::Value::SingleQuotedString(ident),
            )),
            _ => Err(anyhow!("Can't convert to Expr at the moment; {:?}", item)),
        }
    }
}

impl TryFrom<Item> for Vec<sqlparser::ast::Expr> {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::List(items) => Ok(items.iter().map(|x| x.clone().try_into()).try_collect()?),
            _ => Err(anyhow!(
                "Can't convert to Vec<Expr> at the moment; {:?}",
                item
            )),
        }
    }
}

#[test]
fn test_try_from_list_to_vec_expr() {
    let item = Item::List(vec![
        Item::Ident("a".to_owned()),
        Item::Ident("b".to_owned()),
    ]);
    let expr: Vec<sqlparser::ast::Expr> = item.try_into().unwrap();
    assert_eq!(
        expr,
        vec![
            sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new("a")),
            sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new("b"))
        ]
    );
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
    use insta::{assert_debug_snapshot, assert_display_snapshot};
    use serde_yaml::from_str;

    use crate::ast::Pipeline;

    #[test]
    fn test_queries_of_pipeline() {
        // One aggregate, take at the end
        let yaml: &str = r###"
- From:
    - Ident: employees
- Filter:
    - Ident: country
    - Raw: "="
    - String: USA
- Aggregate:
    by:
      - Ident: title
      - Ident: country
    calcs:
      - Func:
          name: average
          args:
            - Ident: salary
          named_args: []
    assigns: []
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
    - String: USA
- Aggregate:
    by:
      - Ident: title
      - Ident: country
    calcs:
      - Func:
          name: average
          args:
            - Ident: salary
          named_args: []
    assigns: []
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
    - String: USA
- Aggregate:
    by:
      - Ident: title
      - Ident: country
    calcs:
      - Func:
          name: average
          args:
            - Ident: salary
          named_args: []
    assigns: []
- Aggregate:
    by:
      - Ident: title
      - Ident: country
    calcs:
      - Func:
          name: average
          args:
            - Ident: salary
          named_args: []
    assigns: []
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
    - String: USA
- Aggregate:
    by:
      - Ident: title
      - Ident: country
    calcs:
      - Func:
          name: average
          args:
            - Ident: salary
          named_args: []
    assigns: []
- Sort:
    - Ident: title
- Take: 20
            "###;

        let pipeline: Pipeline = from_str(yaml).unwrap();
        let select = to_select(&pipeline).unwrap();
        assert_debug_snapshot!(select);
        // TODO: totally wrong but compiles, and we're on our way to fixing it.
        assert_display_snapshot!(select,
            @r###"SELECT TOP (20) TODO: Func { name: "average", args: [Ident("salary")], named_args: [] } FROM employees WHERE country = 'USA' GROUP BY title, country SORT BY title"###
        );
    }
}
