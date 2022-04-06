//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
// The average code quality here is low — we're basically plugging in test
// cases and fixing what breaks, with some occasional refactors. I'm not sure
// that's a terrible approach — the SQL spec is huge, so we're not reasonably
// going to be isomorphically mapping everything back from SQL to PRQL. But it
// does mean we should continue to iterate on this file and refactor things when
// necessary.
use anyhow::{anyhow, bail, Result};
use itertools::Itertools;
use sqlformat::{format, FormatOptions, QueryParams};
use sqlparser::ast::{
    self as sql_ast, Expr, Join, JoinConstraint, JoinOperator, ObjectName, OrderByExpr, Select,
    SelectItem, SetExpr, TableAlias, TableFactor, TableWithJoins, Top,
};
use std::collections::HashMap;

use super::ast::*;
use super::utils::*;
use crate::semantic;
use crate::semantic::SelectedColumns;

/// Translate a PRQL AST into a SQL string.
pub fn translate(query: &Query) -> Result<String> {
    let sql_query = translate_query(query)?;

    let sql_query_string = sql_query.to_string();

    let formatted = format(
        &sql_query_string,
        &QueryParams::default(),
        FormatOptions::default(),
    );
    Ok(formatted)
}

pub fn translate_query(query: &Query) -> Result<sql_ast::Query> {
    // extract tables and the pipeline
    let (tables, functions, pipelines) = separate_pipeline(query)?;

    // load std lib
    let std_lib = load_std_lib()?;
    let functions = [std_lib, functions].concat();

    // combine tables and main pipeline
    let pipeline = pipelines.into_only()?.into();
    let tables = [tables, vec![pipeline]].concat();

    // split to atomics
    let atomics = atomic_tables_of_tables(tables)?;

    // init query context
    let (_, mut context, _) = semantic::process(functions, None)?;

    // materialize each atomic in two stages
    let mut materialized = Vec::new();
    for t in atomics {
        let (stage_1, stage_2) = split_to_stages(t.pipeline);

        let (stage_1, c, select) = semantic::process_pipeline(stage_1, Some(context))?;

        let (stage_2, c, _) = semantic::process_pipeline(stage_2, Some(c))?;
        context = c;

        materialized.push(AtomicTable {
            name: t.name,
            select,
            pipeline: [stage_1, stage_2].concat(),
        });
    }

    // take last table
    if materialized.is_empty() {
        bail!("No tables?");
    }
    let main_query = materialized.remove(materialized.len() - 1);
    let ctes = materialized;

    // convert each of the CTEs
    let ctes: Vec<_> = ctes.into_iter().map(table_to_sql_cte).try_collect()?;

    // convert main query
    let mut main_query = sql_query_of_atomic_table(main_query)?;

    // attach CTEs
    if !ctes.is_empty() {
        main_query.with = Some(sql_ast::With {
            cte_tables: ctes,
            recursive: false,
        });
    }

    Ok(main_query)
}

struct AtomicTable {
    name: String,
    select: SelectedColumns,
    pipeline: Vec<Transform>,
}

pub fn load_std_lib() -> Result<Vec<Node>> {
    use crate::parse;
    let std_lib = include_str!("./stdlib.prql");
    Ok(parse(std_lib)?.nodes)
}

/// Splits an atomic pipeline into two stages:
/// - stage 1, that must contain materialized variables (SELECT, WHERE, GROUP BY, HAVING)
/// - stage 2, that must contain variables by name (ORDER BY)
fn split_to_stages(mut pipeline: Pipeline) -> (Pipeline, Pipeline) {
    let (stage_1, stage_2) = pipeline
        .drain(..)
        .partition(|t| !matches!(t, Transform::Sort(_)));
    (stage_1, stage_2)
}

fn separate_pipeline(query: &Query) -> Result<(Vec<Table>, Vec<Node>, Vec<Pipeline>)> {
    let mut tables: Vec<Table> = Vec::new();
    let mut functions: Vec<Node> = Vec::new();
    let mut pipelines: Vec<Pipeline> = Vec::new();
    for node in &query.nodes {
        match node {
            Node {
                item: Item::Table(t),
                ..
            } => tables.push(t.clone()),
            Node {
                item: Item::FuncDef(_),
                ..
            } => functions.push(node.clone()),
            Node {
                item: Item::Pipeline(p),
                ..
            } => pipelines.push(p.clone()),
            i => bail!("Unexpected item on top level: {i:?}"),
        }
    }
    Ok((tables, functions, pipelines))
}

fn table_to_sql_cte(table: AtomicTable) -> Result<sql_ast::Cte> {
    let alias = sql_ast::TableAlias {
        name: Item::Ident(table.name.clone()).try_into()?,
        columns: vec![],
    };
    Ok(sql_ast::Cte {
        alias,
        query: sql_query_of_atomic_table(table)?,
        from: None,
    })
}

// TODO: currently we can't implement a TryFrom for Pipeline because it's a type
// alias. Possibly at some point we should turn it into a wrapper type. (We can
// still implement Iter & IntoIterator, though.)
//
// impl TryFrom<Pipeline> for sqlparser::sql_ast::Query {
//     type Error = anyhow::Error;
//     fn try_from(&pipeline: Pipeline) -> Result<sqlparser::sql_ast::Query> {

fn table_factor_of_table_ref(table_ref: &TableRef) -> TableFactor {
    TableFactor::Table {
        name: ObjectName(vec![Item::Ident(table_ref.name.clone())
            .try_into()
            .unwrap()]),
        alias: table_ref.alias.clone().map(|a| TableAlias {
            name: Item::Ident(a).try_into().unwrap(),
            columns: vec![],
        }),
        args: vec![],
        with_hints: vec![],
    }
}

fn sql_query_of_atomic_table(table: AtomicTable) -> Result<sql_ast::Query> {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on the caller for that?

    let mut from = table
        .pipeline
        .iter()
        .filter_map(|t| match t {
            Transform::From(table_ref) => Some(TableWithJoins {
                relation: table_factor_of_table_ref(table_ref),
                joins: vec![],
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    let joins = table
        .pipeline
        .iter()
        .filter(|t| matches!(t, Transform::Join { .. }))
        .map(|t| match t {
            Transform::Join { side, with, on } => {
                let use_using = (on.iter().map(|x| &x.item)).all(|x| matches!(x, Item::Ident(_)));

                let constraint = if use_using {
                    JoinConstraint::Using(
                        on.iter()
                            .map(|x| x.item.clone().try_into())
                            .collect::<Result<Vec<_>>>()?,
                    )
                } else {
                    Item::Expr(on.to_vec()).try_into().map(JoinConstraint::On)?
                };

                Ok(Join {
                    relation: table_factor_of_table_ref(with),
                    join_operator: match *side {
                        JoinSide::Inner => JoinOperator::Inner(constraint),
                        JoinSide::Left => JoinOperator::LeftOuter(constraint),
                        JoinSide::Right => JoinOperator::RightOuter(constraint),
                        JoinSide::Full => JoinOperator::FullOuter(constraint),
                    },
                })
            }
            _ => unreachable!(),
        })
        .collect::<Result<Vec<_>>>()?;
    if !joins.is_empty() {
        if let Some(from) = from.last_mut() {
            from.joins = joins;
        } else {
            return Err(anyhow!("Cannot use `join` without `from`"));
        }
    }

    // Split the pipeline into before & after the aggregate
    let (before, after) = table.pipeline.split_at(
        table
            .pipeline
            .iter()
            .position(|t| matches!(t, Transform::Aggregate { .. }))
            .unwrap_or(table.pipeline.len()),
    );
    // Convert the filters in a pipeline into an Expr
    fn filter_of_pipeline(pipeline: &[Transform]) -> Result<Option<Expr>> {
        let filters: Vec<Filter> = pipeline
            .iter()
            .filter_map(|t| match t {
                Transform::Filter(filter) => Some(filter),
                _ => None,
            })
            .cloned()
            .collect();

        Ok(if !filters.is_empty() {
            Some((Item::Expr(Filter::combine_filters(filters).0)).try_into()?)
        } else {
            None
        })
    }
    // Find the filters that come before the aggregation.
    let where_ = filter_of_pipeline(before).unwrap();
    let having = filter_of_pipeline(after).unwrap();

    let take = table
        .pipeline
        .iter()
        .filter_map(|t| match t {
            Transform::Take(_) => Some(t.clone().try_into()),
            _ => None,
        })
        .last()
        .transpose()?;

    // Find the final sort (none of the others affect the result, and can be discarded).
    let order_by = table
        .pipeline
        .iter()
        .filter_map(|t| match t {
            Transform::Sort(items) => Some(Item::Expr(items.to_owned()).try_into().map(|x| {
                vec![OrderByExpr {
                    expr: x,
                    asc: None,
                    nulls_first: None,
                }]
            })),
            _ => None,
        })
        .last()
        .unwrap_or(Ok(vec![]))?;

    let aggregate = table
        .pipeline
        .iter()
        .find(|t| matches!(t, Transform::Aggregate { .. }));
    let group_bys: Vec<Node> = match aggregate {
        Some(Transform::Aggregate { by, .. }) => by.clone(),
        None => vec![],
        _ => unreachable!("Expected an aggregate transformation"),
    };
    let group_by = Node::into_list_of_nodes(group_bys).item.try_into()?;

    Ok(sql_ast::Query {
        body: SetExpr::Select(Box::new(Select {
            distinct: false,
            top: None,
            projection: (table.select.0.into_iter())
                .map(|n| n.item.try_into())
                .try_collect()?,
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
        // TODO: when should this be `TOP` vs `LIMIT` (which is on the `Query` object?)
        limit: take,
        offset: None,
        fetch: None,
    })
}

/// Convert a pipeline into a number of pipelines which can each "fit" into a SELECT.
fn atomic_pipelines_of_pipeline(pipeline: &Pipeline) -> Result<Vec<Pipeline>> {
    // Insert a cut, when we find transformation that out of order:
    // - joins,
    // - filters (for WHERE)
    // - aggregate (max 1x)
    // - sort (max 1x)
    // - filters (for HAVING)
    // - take (max 1x)
    //
    // Select and derive should already be extracted during resolving phase.
    //
    // So we loop through the Pipeline, and cut it into cte-sized pipelines,
    // which we'll then compose together.

    let mut counts: HashMap<&str, u32> = HashMap::new();
    let mut splits = vec![0];
    for (i, transform) in pipeline.iter().enumerate() {
        
        let split = match transform.name() {
            "join" => {
                counts.get("filter").is_some()
                || counts.get("aggregate").is_some()
                || counts.get("sort").is_some()
                || counts.get("take").is_some()
            },
            "aggregate" => {
                counts.get("aggregate").is_some()
                || counts.get("sort").is_some()
                || counts.get("take").is_some()
            }
            "filter" => {
                counts.get("take").is_some()
            },
            "sort" => {
                counts.get("sort").is_some()
                || counts.get("take").is_some()
            },
            "take" => {
                counts.get("take").is_some()
            },
            
            _ => false,
        };

        if split {
            splits.push(i);
            counts.clear();
        }

        *counts.entry(transform.name()).or_insert(0) += 1;
    }

    splits.push(pipeline.len());
    let ctes = (0..splits.len() - 1)
        .map(|i| pipeline[splits[i]..splits[i + 1]].to_vec())
        .filter(|x| !x.is_empty())
        .collect();
    Ok(ctes)
}

/// Converts a series of tables into a series of atomic tables, by putting the
/// next pipeline's `from` as the current pipelines's table name.
fn atomic_tables_of_tables(tables: Vec<Table>) -> Result<Vec<Table>> {
    let mut atomics = Vec::new();
    let mut index = 0;
    for t in tables {
        // split table into atomics
        let mut t_atomics: Vec<_> = atomic_pipelines_of_pipeline(&t.pipeline)?
            .into_iter()
            .map(Table::from)
            .collect();

        let (last, ctes) = t_atomics
            .split_last_mut()
            .ok_or_else(|| anyhow!("No pipelines?"))?;

        // generate table names for all but last table
        let mut last_name = None;
        for cte in ctes {
            prepend_with_from(&mut cte.pipeline, &last_name);

            cte.name = format!("table_{index}");
            index += 1;
            last_name = Some(cte.name.clone());
        }

        // use original table name
        prepend_with_from(&mut last.pipeline, &last_name);
        last.name = t.name;

        atomics.extend(t_atomics);
    }
    Ok(atomics)
}

fn prepend_with_from(pipeline: &mut Pipeline, last_name: &Option<String>) {
    if let Some(last_name) = last_name {
        let from = Transform::From(TableRef {
            name: last_name.clone(),
            alias: None,
        });
        pipeline.insert(0, from);
    }
}

/// Combines filters by putting them in parentheses and then joining them with `and`.
// Feels hacky — maybe this should be operation on a different level.
impl Filter {
    #[allow(unstable_name_collisions)] // Same behavior as the std lib; we can remove this + itertools when that's released.
    fn combine_filters(filters: Vec<Filter>) -> Filter {
        Filter(
            filters
                .into_iter()
                .map(|f| Item::Expr(f.0).into())
                .intersperse(Item::Raw("and".to_owned()).into())
                .collect(),
        )
    }
}

impl TryFrom<Item> for SelectItem {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        Ok(match item {
            Item::Expr(_) | Item::SString(_) | Item::Ident(_) | Item::Raw(_) => {
                SelectItem::UnnamedExpr(TryInto::<Expr>::try_into(item)?)
            }
            Item::NamedExpr(named) => SelectItem::ExprWithAlias {
                alias: sql_ast::Ident {
                    value: named.name,
                    quote_style: None,
                },
                expr: named.expr.item.try_into()?,
            },
            _ => bail!("Can't convert to SelectItem at the moment; {:?}", item),
        })
    }
}

impl TryFrom<Transform> for Expr {
    type Error = anyhow::Error;
    fn try_from(transformation: Transform) -> Result<Self> {
        match transformation {
            Transform::Take(take) => Ok(
                // TODO: implement for number
                Item::Raw(take.to_string()).try_into()?,
            ),
            _ => Err(anyhow!(
                "Expr transformation currently only supported for Take"
            )),
        }
    }
}

impl TryFrom<Transform> for Top {
    type Error = anyhow::Error;
    fn try_from(transformation: Transform) -> Result<Self> {
        match transformation {
            Transform::Take(take) => Ok(Top {
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
        Ok(match item {
            Item::Ident(ident) => Expr::Identifier(sql_ast::Ident::new(ident)),
            Item::Raw(raw) => Expr::Identifier(sql_ast::Ident::new(raw.to_uppercase())),
            // For expressions like `country = "USA"`, we take each one, convert
            // it, and put spaces between them. It's a bit hacky — we could
            // convert each term to a SQL AST item, but it works for the moment.
            //
            // (one question is whether we need to surround `Expr` with parentheses?)
            Item::Expr(items) => {
                Expr::Identifier(sql_ast::Ident::new(
                    items
                        .into_iter()
                        .map(|node| TryInto::<Expr>::try_into(node.item).unwrap())
                        .collect::<Vec<Expr>>()
                        .iter()
                        .map(|x| x.to_string())
                        // Currently a hack, but maybe OK, since we don't
                        // need to parse every single expression into sqlparser ast.
                        .join(" "),
                ))
            }
            Item::String(s) => Expr::Value(sql_ast::Value::SingleQuotedString(s)),
            // Fairly hacky — convert everything to a string, then concat it,
            // then convert to Expr. We can't use the `Terms` code above
            // since we don't want to intersperse with spaces.
            Item::SString(s_string_items) => {
                let string = s_string_items
                    .into_iter()
                    .map(|s_string_item| match s_string_item {
                        SStringItem::String(string) => Ok(string),
                        SStringItem::Expr(node) => {
                            TryInto::<Expr>::try_into(node.item).map(|expr| expr.to_string())
                        }
                    })
                    .collect::<Result<Vec<String>>>()?
                    .join("");
                Item::Ident(string).try_into()?
            }
            _ => bail!("Can't convert to Expr at the moment; {item:?}"),
        })
    }
}
impl TryFrom<Item> for Vec<Expr> {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::List(_) => Ok(Into::<Node>::into(item)
                // TODO: implement for non-single item ListItems
                .into_inner_list_nodes()?
                .into_iter()
                .map(|x| x.item.try_into())
                .try_collect()?),
            _ => Err(anyhow!(
                "Can't convert to Vec<Expr> at the moment; {item:?}"
            )),
        }
    }
}
impl TryFrom<Item> for sql_ast::Ident {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sql_ast::Ident::new(ident)),
            Item::Raw(ident) => Ok(sql_ast::Ident::new(ident)),
            _ => Err(anyhow!("Can't convert to Ident at the moment; {item:?}")),
        }
    }
}
impl From<Pipeline> for Table {
    fn from(pipeline: Pipeline) -> Self {
        Table {
            name: String::default(),
            pipeline,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::parser::parse;
    use insta::{
        assert_debug_snapshot, assert_display_snapshot, assert_snapshot, assert_yaml_snapshot,
    };
    use serde_yaml::from_str;

    #[test]
    fn test_try_from_s_string_to_expr() -> Result<()> {
        let ast: Node = from_str(
            r"
SString:
 - String: SUM(
 - Expr:
     Expr:
       - Ident: col
 - String: )
",
        )?;
        let expr: Expr = ast.item.try_into()?;
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
            ListItem(Item::Ident("a".to_owned()).into()),
            ListItem(Item::Ident("b".to_owned()).into()),
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
    fn test_ctes_of_pipeline_1() -> Result<()> {
        // One aggregate, take at the end
        let yaml: &str = r###"
- From:
    name: employees
    alias: ~
- Filter:
    - Ident: country
    - Raw: "="
    - String: USA
- Aggregate:
    by:
    - Ident: title
    - Ident: country
    select:
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
        Ok(())
    }

    #[test]
    fn test_ctes_of_pipeline_2() -> Result<()> {
        // One aggregate, but take at the top
        let yaml: &str = r###"
    - From:
        name: employees
        alias: ~
    - Take: 20
    - Filter:
        - Ident: country
        - Raw: "="
        - String: USA
    - Aggregate:
        by:
        - Ident: title
        - Ident: country
        select:
          - Ident: average
          - Ident: salary
        assigns: []
    - Sort:
        - Ident: title
        "###;

        let pipeline: Pipeline = from_str(yaml)?;
        let queries = atomic_pipelines_of_pipeline(&pipeline)?;
        assert_eq!(queries.len(), 2);
        Ok(())
    }

    #[test]
    fn test_ctes_of_pipeline_3() -> Result<()> {
        // A take, then two aggregates
        let yaml: &str = r###"
    - From:
        name: employees
        alias: ~
    - Take: 20
    - Filter:
        - Ident: country
        - Raw: "="
        - String: USA
    - Aggregate:
        by:
        - Ident: title
        - Ident: country
        select:
        - Ident: average
        - Ident: salary
        assigns: []
    - Aggregate:
        by:
        - Ident: title
        - Ident: country
        select:
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
    fn test_ctes_of_pipeline_4() -> Result<()> {
        // A take, then a select
        let yaml: &str = r###"
    - From:
        name: employees
        alias: ~
    - Take: 20
    - Select:
        - Ident: first_name
        "###;

        let pipeline: Pipeline = from_str(yaml)?;
        let queries = atomic_pipelines_of_pipeline(&pipeline)?;
        assert_eq!(queries.len(), 1);
        Ok(())
    }

    #[test]
    fn test_sql_of_ast_1() -> Result<()> {
        let query: Query = parse(
            r###"
        from employees
        filter country = "USA"
        aggregate by:[title, country] [
            average salary
        ]
        sort title
        take 20
        "###,
        )?;

        let sql = translate(&query)?;
        assert_display_snapshot!(sql,
            @r###"
        SELECT
          title,
          country,
          AVG(salary)
        FROM
          employees
        WHERE
          country = 'USA'
        GROUP BY
          title,
          country
        ORDER BY
          title
        LIMIT
          20
        "###
        );
        assert!(sql.to_lowercase().contains(&"avg(salary)".to_lowercase()));
        Ok(())
    }

    #[test]
    fn test_sql_of_ast_2() -> Result<()> {
        let query: Query = from_str(
            r###"
            nodes:
            - Pipeline:
                - From:
                    name: employees
                    alias: ~
                - Aggregate:
                    by: []
                    select:
                    - SString:
                        - String: count(
                        - Expr:
                            Ident: salary
                        - String: )
                    - NamedExpr:
                        name: sum_salary
                        expr:
                            Ident: salary
                - Filter:
                    - Ident: salary
                    - Raw: ">"
                    - Raw: "100"
        "###,
        )?;
        let sql = translate(&query)?;
        assert_snapshot!(sql, @r###"
        SELECT
          count(salary),
          salary AS sum_salary
        FROM
          employees
        HAVING
          salary > 100
        "###);
        assert!(sql.to_lowercase().contains(&"having".to_lowercase()));

        Ok(())
    }

    #[test]
    fn test_prql_to_sql_1() -> Result<()> {
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
        let sql = translate(&query)?;
        assert_display_snapshot!(sql,
            @r###"
        SELECT
          count(salary),
          sum(salary)
        FROM
          employees
        "###
        );
        Ok(())
    }

    #[test]
    fn test_prql_to_sql_2() -> Result<()> {
        let query = parse(
            r#"
from employees
filter country = "USA"                           # Each line transforms the previous result.
derive [                                         # This adds columns / variables.
  gross_salary: salary + payroll_tax,
  gross_cost:   gross_salary + benefits_cost    # Variables can use other variables.
]
filter gross_cost > 0
aggregate by:[title, country] [                  # `by` are the columns to group by.
    average salary,                              # These are aggregation calcs run on each group.
    sum     salary,
    average gross_salary,
    sum     gross_salary,
    average gross_cost,
    sum_gross_cost: sum gross_cost,
    ct: count,
]
sort sum_gross_cost
filter ct > 200
take 20
"#,
        )?;

        let sql = translate(&query)?;
        assert_display_snapshot!(sql);
        Ok(())
    }

    #[test]
    fn test_prql_to_sql_table() -> Result<()> {
        // table
        let query = parse(
            r#"
        table newest_employees = (
            from employees
            sort tenure
            take 50
        )
        table average_salaries = (
            from salaries
            aggregate by:country [
                average_country_salary: average salary
            ]
        )
        from newest_employees
        join average_salaries [country]
        select [name, salary, average_country_salary]
    "#,
        )?;
        let sql = translate(&query)?;
        assert_display_snapshot!(sql,
            @r###"
        WITH newest_employees AS (
          SELECT
            *
          FROM
            employees
          ORDER BY
            tenure
          LIMIT
            50
        ), average_salaries AS (
          SELECT
            country,
            AVG(salary) AS average_country_salary
          FROM
            salaries
          GROUP BY
            country
        )
        SELECT
          name,
          salary,
          average_country_salary
        FROM
          newest_employees
          JOIN average_salaries USING(country)
        "###
        );

        Ok(())
    }

    #[test]
    fn test_nonatomic() -> Result<()> {
        // A take, then two aggregates
        let query: Query = parse(
            r###"
            from employees
            take 20
            filter country = "USA"
            aggregate by:[title, country] [
                average salary
            ]
            aggregate by:[title, country] [
                average salary
            ]
            sort sum_gross_cost
        "###,
        )?;

        assert_display_snapshot!((translate(&query)?), @r###"
        WITH table_0 AS (
          SELECT
            *
          FROM
            employees
          LIMIT
            20
        ), table_1 AS (
          SELECT
            title,
            country,
            AVG(salary)
          FROM
            table_0
          WHERE
            country = 'USA'
          GROUP BY
            title,
            country
        )
        SELECT
          title,
          country,
          AVG(salary)
        FROM
          table_1
        GROUP BY
          title,
          country
        ORDER BY
          sum_gross_cost
        "###);

        Ok(())
    }

    #[test]
    /// Confirm a nonatomic table works.
    fn test_nonatomic_table() -> Result<()> {
        // A take, then two aggregates
        let query = parse(
            r###"
        table a = (
            from employees
            take 50
            aggregate [s"count(*)"]
        )
        from a
        join b [country]
        select [name, salary, average_country_salary]
"###,
        )?;

        assert_display_snapshot!((translate(&query)?), @r###"
        WITH table_0 AS (
          SELECT
            *
          FROM
            employees
          LIMIT
            50
        ), a AS (
          SELECT
            count(*)
          FROM
            table_0
        )
        SELECT
          name,
          salary,
          average_country_salary
        FROM
          a
          JOIN b USING(country)
        "###);

        Ok(())
    }

    #[test]
    #[should_panic]
    fn test_table_references() {
        let prql = r#"
from employees
take 10
join salaries [employees.employee_id=salaries.employee_id]
        "#;
        let result = parse(prql).and_then(|x| translate(&x)).unwrap();
        assert_display_snapshot!(result, @r###"
        WITH table_0 AS (
          SELECT
            *
          FROM
            employees
          LIMIT
            10
        )
        SELECT
          *
        FROM
          table_0
          JOIN salaries ON employees.employee_id = salaries.employee_id
        "###);
        assert!(!result.contains("employees.employee_id"));
    }

    #[test]
    fn test_table_alias() -> Result<()> {
        // Alias on from
        let query: Query = parse(
            r###"
            from e: employees
            join salaries side:left [salaries.emp_no = e.emp_no]
            aggregate by:[e.emp_no] [
              emp_salary: average salary
            ]
            select [e.emp_no, emp_salary]
        "###,
        )?;

        assert_display_snapshot!((translate(&query)?), @r###"
        SELECT
          e.emp_no,
          AVG(salary) AS emp_salary
        FROM
          employees AS e
          LEFT JOIN salaries ON salaries.emp_no = e.emp_no
        GROUP BY
          e.emp_no
        "###);
        Ok(())
    }
}
