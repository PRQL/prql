// The average code quality here is quite low — we're basically plugging in test
// cases and fixing what breaks, with some occasional refactors. I'm not sure
// that's a terrible approach — the SQL spec is huge, so we're not reasonably
// going to be isomorphically mapping everything back from SQL to PRQL. But it
// does mean we should continue to iterate on this file and refactor things when
// necessary.

use super::ast::*;
use super::utils::*;
use anyhow::{anyhow, Result};
use itertools::Itertools;
use sqlformat::{format, FormatOptions, QueryParams};
use sqlparser::ast::{
    Expr, ObjectName, OrderByExpr, Select, SelectItem, SetExpr, TableFactor, TableWithJoins, Top,
};
use std::collections::HashMap;

/// Convert a PRQL AST to SQL.
pub fn sql_of_ast(ast: &Item) -> Result<String> {
    let sql_query: sqlparser::ast::Query = ast.as_query().unwrap().clone().try_into()?;

    let sql_query_string = sql_query.to_string();

    let formatted = format(
        &sql_query_string,
        &QueryParams::default(),
        FormatOptions::default(),
    );
    Ok(formatted)
}

impl TryFrom<Query> for sqlparser::ast::Query {
    type Error = anyhow::Error;
    // TODO: implement for Table (though I don't think we can implement the
    // trait, since it goes into a different type), add tests for pipelines that
    // need to be split up into CTEs.
    fn try_from(query: Query) -> Result<Self> {
        // We don't compile functions into SQL.
        let compilable: Vec<Item> = query
            .items
            .iter()
            .filter(|item| !matches!(item, Item::Function(_)))
            .cloned()
            .collect();

        let items: Vec<Item> = compilable
            .iter()
            .map(|item| {
                match item {
                    Item::Pipeline(pipeline) => {
                        let ctes = atomic_pipelines_of_pipeline(pipeline)?;
                        if ctes.len() > 1 {
                            tables_of_pipelines(ctes).map(|x| {
                                Item::Items(x.into_iter().map(Item::Table).collect::<Vec<Item>>())
                            })
                        } else {
                            ctes.into_only().map(Item::Pipeline)
                        }
                    }
                    // TODO
                    Item::Table(_) => unimplemented!(),
                    _ => unreachable!(),
                }
            })
            .try_collect()?;

        let query = items
            .iter()
            .map(|item| match item {
                Item::Pipeline(pipeline) => sql_query_of_pipeline(pipeline),
                _ => unimplemented!(),
            })
            .collect::<Result<Vec<sqlparser::ast::Query>>>()?
            .into_only()?;

        Ok(query)
    }
}

// TODO: currently we can't implement a TryFrom for Pipeline because it's a type
// alias. Possibly at some point we should turn it into a wrapper type. (We can
// still implement Iter & IntoIterator, though.)
//
// impl TryFrom<Pipeline> for sqlparser::ast::Query {
//     type Error = anyhow::Error;
//     fn try_from(&pipeline: Pipeline) -> Result<sqlparser::ast::Query> {

fn sql_query_of_pipeline(pipeline: &Pipeline) -> Result<sqlparser::ast::Query> {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on the caller for that?
    // TODO: this doesn't handle joins at all yet.

    // Alternatively we could
    // - Do a single pass, but we need to split by before & after the
    //   `aggregate`, even before considering joins. If we did a single pass, do
    //   something like: group_pairs from
    //   https://stackoverflow.com/a/65394297/3064736 let grouped =
    //   group_pairs(pipeline.iter().map(|t| (t.name, t))); let from =
    //   grouped.get(&TransformationType::From).unwrap().first().unwrap().clone();

    let from = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::From(ident) => Some(TableWithJoins {
                relation: TableFactor::Table {
                    name: ObjectName(vec![Item::Ident(ident.clone()).try_into().unwrap()]),
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
    fn filter_of_pipeline(pipeline: &[Transformation]) -> Result<Option<Expr>> {
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
            Some((Item::Terms(Filter::combine_filters(filters).0)).try_into()?)
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
        // Swap result & option.
        .map_or(Ok(None), |r| r.map(Some))?;

    // Find the final sort (none of the others affect the result, and can be discarded).
    let order_by = pipeline
        .iter()
        .filter_map(|t| match t {
            Transformation::Sort(items) => {
                Some(Item::Terms(items.to_owned()).try_into().map(|x| {
                    vec![OrderByExpr {
                        expr: x,
                        asc: None,
                        nulls_first: None,
                    }]
                }))
            }
            _ => None,
        })
        .last()
        .unwrap_or(Ok(vec![]))?;

    let aggregate = pipeline
        .iter()
        .find(|t| matches!(t, Transformation::Aggregate { .. }));
    let (group_bys, select_from_aggregate): (Vec<Item>, Option<Vec<SelectItem>>) = match aggregate {
        Some(Transformation::Aggregate { by, calcs, assigns }) => (
            by.clone(),
            // This is chaining a) the assigns (such as `sum_salary: sum
            // salary`), and b) the calcs (such as `sum salary`); and converting
            // them into SelectItems.
            Some(
                assigns
                    .iter()
                    .map(|x| x.clone().try_into())
                    .chain(calcs.iter().map(|x| x.clone().try_into()))
                    .try_collect()?,
            ),
        ),
        None => (vec![], None),
        _ => unreachable!("Expected an aggregate transformation"),
    };
    let group_by = Item::into_list_of_items(group_bys).try_into()?;
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
        .map_or(Ok(None), |r| r.map(Some))?;

    let select = [
        Some(select_from_derive),
        select_from_select,
        select_from_aggregate,
    ]
    .into_iter()
    // TODO: should we do the option flattening here or in each of the selects?
    .flatten()
    .flatten()
    .collect();

    Ok(sqlparser::ast::Query {
        body: SetExpr::Select(Box::new(Select {
            distinct: false,
            // TODO: when should this be `TOP` vs `LIMIT` (which is on the `Query` object?)
            top: take,
            projection: select,
            from,
            group_by,
            having,
            selection: where_,
            sort_by: vec![],
            lateral_views: vec![],
            distribute_by: vec![],
            cluster_by: vec![],
        })),
        order_by,
        with: None,
        limit: None,
        offset: None,
        fetch: None,
    })
}

// Alternatively this could be a `TryInto` impl?
/// Convert a pipeline into a number of pipelines which can each "fit" into a CTE.
fn atomic_pipelines_of_pipeline(pipeline: &Pipeline) -> Result<Vec<Pipeline>> {
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

    // TODO: this used to work but we get a borrow checker error when using `str`.
    // let mut counts: HashMap<&str, u32> = HashMap::new();
    let mut counts: HashMap<String, u32> = HashMap::new();

    let mut current_cte: Pipeline = vec![];

    // This seems inelegant! I'm sure there's a better way to do this, though
    // note the constraints from above.
    for transformation in pipeline {
        let transformation = transformation.to_owned();
        if transformation.name() == "aggregate" && counts.get("aggregate") == Some(&1) {
            // push_current_cte()
            // We have a new CTE
            ctes.push(current_cte);
            current_cte = vec![];
            counts.clear();
        }

        // As above re `.to_owned`.
        *counts.entry(transformation.name().to_owned()).or_insert(0) += 1;
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

    Ok(ctes)
}

/// Converts a series of pipelines into a series of tables, by putting the
/// next pipeline's `from` as the current pipelines's table name.
fn tables_of_pipelines(pipelines: Vec<Pipeline>) -> Result<Vec<Table>> {
    let mut tables = vec![];
    for (n, mut pipeline) in pipelines.into_iter().enumerate() {
        if n > 0 {
            pipeline.insert(0, Transformation::From(format!("table_{}", n - 1)));
        }
        tables.push(Table {
            name: "table_".to_owned() + &n.to_string(),
            pipeline: pipeline.clone(),
        });
    }
    Ok(tables)
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

impl TryFrom<Item> for SelectItem {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::SString(_) | Item::Ident(_) | Item::Terms(_) => {
                Ok(SelectItem::UnnamedExpr(TryInto::<Expr>::try_into(item)?))
            }
            _ => Err(anyhow!(
                "Can't convert to SelectItem at the moment; {:?}",
                item
            )),
        }
    }
}

impl TryFrom<Transformation> for Top {
    type Error = anyhow::Error;
    fn try_from(transformation: Transformation) -> Result<Self> {
        match transformation {
            Transformation::Take(take) => Ok(Top {
                // TODO: implement for number
                quantity: Some(Item::Raw(take.to_string()).try_into()?),
                with_ties: false,
                percent: false,
            }),
            _ => Err(anyhow!("Top transformation only supported for Take")),
        }
    }
}

impl TryFrom<Item> for Expr {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(Expr::Identifier(
                sqlparser::ast::Ident::new(ident),
            )),
            Item::Raw(ident) => Ok(Expr::Identifier(
                sqlparser::ast::Ident::new(ident),
            )),
            // For expressions like `country = "USA"`, we take each one, convert
            // it, and put spaces between them. It's a bit hacky — we could
            // convert each term to a SQL AST item, but it works for the moment.
            //
            // (one question is why that is coming as a `Terms` rather an `Items`?)
            Item::Terms(items) => Ok(Expr::Identifier(
                sqlparser::ast::Ident::new(
                    items
                        .into_iter()
                        .map(|item| TryInto::<Expr>::try_into(item).unwrap())
                        .collect::<Vec<Expr>>()
                        .iter()
                        .map(|x| x.to_string())
                        // Currently a hack, but maybe OK, since we don't
                        // need to parse every single expression into sqlparser ast.
                        .join(" "),
                ),
            )),
            Item::String(ident) => Ok(Expr::Value(
                sqlparser::ast::Value::SingleQuotedString(ident),
            )),
            // Fairly hacky — convert everything to a string, then concat it,
            // then convert to Expr. We can't use the `Terms` code above
            // since we don't want to intersperse with spaces.
            Item::SString(s_string_items) => {
                let string = s_string_items
                    .into_iter()
                    .map(|s_string_item| match s_string_item {
                        SStringItem::String(string) => Ok(string),
                        SStringItem::Expr(item) => TryInto::<Expr>::try_into(item)
                            .map(|expr| expr.to_string()),
                    })
                    .collect::<Result<Vec<String>>>()?
                    .join("");
                Item::Ident(string).try_into()
            }
            Item::Items(_) => Err(anyhow!(
                "Not yet implemented for `Items`; (something we probably need to do, see notes above); {item:?}"
            )),
            _ => Err(anyhow!("Can't convert to Expr at the moment; {item:?}")),
        }
    }
}
impl TryFrom<Item> for Vec<Expr> {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::List(_) => Ok(item
                // TODO: implement for non-single item ListItems
                .into_inner_list_single_items()?
                .into_iter()
                .map(|x| x.try_into())
                .try_collect()?),
            _ => Err(anyhow!(
                "Can't convert to Vec<Expr> at the moment; {item:?}"
            )),
        }
    }
}
impl TryFrom<Item> for sqlparser::ast::Ident {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sqlparser::ast::Ident::new(ident)),
            Item::Raw(ident) => Ok(sqlparser::ast::Ident::new(ident)),
            _ => Err(anyhow!("Can't convert to Ident at the moment; {item:?}")),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::compiler::compile;
    use crate::parser::parse;
    use insta::{assert_debug_snapshot, assert_display_snapshot};
    use serde_yaml::from_str;

    #[test]
    fn test_try_from_s_string_to_expr() -> Result<()> {
        use insta::assert_yaml_snapshot;
        use serde_yaml::from_str;
        let yaml: &str = r"
SString:
 - String: SUM(
 - Expr:
     Terms:
       - Ident: col
 - String: )
";
        let ast: Item = from_str(yaml)?;
        let expr: Expr = ast.try_into()?;
        assert_yaml_snapshot!(
            expr, @r###"
    ---
    Identifier:
      value: SUM(col)
      quote_style: ~
    "###
        );
        Ok(())
    }

    #[test]
    fn test_try_from_list_to_vec_expr() -> Result<()> {
        let item = Item::List(vec![
            ListItem(vec![Item::Ident("a".to_owned())]),
            ListItem(vec![Item::Ident("b".to_owned())]),
        ]);
        let expr: Vec<Expr> = item.try_into()?;
        assert_debug_snapshot!(expr, @r###"
        [
            Identifier(
                Ident {
                    value: "a",
                    quote_style: None,
                },
            ),
            Identifier(
                Ident {
                    value: "b",
                    quote_style: None,
                },
            ),
        ]
        "###);
        Ok(())
    }

    #[test]
    fn test_ctes_of_pipeline() -> Result<()> {
        // One aggregate, take at the end
        let yaml: &str = r###"
- From: employees
- Filter:
    - Ident: country
    - Raw: "="
    - String: USA
- Aggregate:
    by:
    - Ident: title
    - Ident: country
    calcs:
    - Terms:
        - Ident: average
        - Ident: salary
    assigns: []
- Sort:
    - Ident: title
- Take: 20
        "###;

        let pipeline: Pipeline = from_str(yaml)?;
        let queries = atomic_pipelines_of_pipeline(&pipeline)?;
        assert_eq!(queries.len(), 1);

        // One aggregate, but take at the top
        let yaml: &str = r###"
    - From: employees
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
        - Terms:
            - Ident: average
            - Ident: salary
        assigns: []
    - Sort:
        - Ident: title
        "###;

        let pipeline: Pipeline = from_str(yaml)?;
        let queries = atomic_pipelines_of_pipeline(&pipeline)?;
        assert_eq!(queries.len(), 2);

        // A take, then two aggregates
        let yaml: &str = r###"
    - From: employees
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
        - Terms:
            - Ident: average
            - Ident: salary
        assigns: []
    - Aggregate:
        by:
        - Ident: title
        - Ident: country
        calcs:
        - Terms:
            - Ident: average
            - Ident: salary
        assigns: []
    - Sort:
        - Ident: sum_gross_cost

        "###;

        let pipeline: Pipeline = from_str(yaml)?;
        let queries = atomic_pipelines_of_pipeline(&pipeline)?;
        assert_eq!(queries.len(), 3);
        Ok(())
    }

    #[test]
    fn test_sql_of_ast() -> Result<()> {
        let yaml: &str = r###"
Query:
  items:
    - Pipeline:
      - From: employees
      - Filter:
          - Ident: country
          - Raw: "="
          - String: USA
      - Aggregate:
          by:
          - Ident: title
          - Ident: country
          calcs:
          - SString:
              - String: AVG(
              - Expr:
                  Ident: salary
              - String: )
          assigns: []
      - Sort:
          - Ident: title
      - Take: 20
            "###;

        let pipeline: Item = from_str(yaml)?;
        let select = sql_of_ast(&pipeline)?;
        assert_display_snapshot!(select,
            @r###"
        SELECT
          TOP (20) AVG(salary)
        FROM
          employees
        WHERE
          country = 'USA'
        GROUP BY
          title,
          country
        ORDER BY
          title
        "###
        );
        assert!(select
            .to_lowercase()
            .contains(&"avg(salary)".to_lowercase()));

        Ok(())
    }

    #[test]
    fn test_prql_to_sql() -> Result<()> {
        let query = parse(
            r#"
    func count x = s"count({x})"
    func sum x = s"sum({x})"

    from employees
    aggregate [
      count salary,
      sum salary,
    ]
    "#,
        )?;
        let ast = compile(query)?;
        let sql = sql_of_ast(&ast)?;
        assert_display_snapshot!(sql,
            @r###"
        SELECT
          count(salary),
          sum(salary)
        FROM
          employees
        "###
        );

        let query = parse(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    count: count,
]
sort sum_gross_cost
filter count > 200
take 20
"#,
        )?;

        let ast = compile(query)?;
        let sql = sql_of_ast(&ast)?;
        assert_display_snapshot!(sql);

        Ok(())
    }
}
