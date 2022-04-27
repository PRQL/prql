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
    self as sql_ast, Expr, Function, FunctionArg, FunctionArgExpr, Join, JoinConstraint,
    JoinOperator, ObjectName, OrderByExpr, Select, SelectItem, SetExpr, TableAlias, TableFactor,
    TableWithJoins, Top,
};
use sqlparser::ast::{DateTimeField, Value};
use std::collections::HashMap;

use crate::ast::JoinFilter;
use crate::ast::*;
use crate::error::{Error, Reason};
use crate::semantic::Context;

use super::materializer::MaterializationContext;
use super::{un_group, MaterializedFrame};

/// Translate a PRQL AST into a SQL string.
pub fn translate(query: Query, context: Context) -> Result<String> {
    let sql_query = translate_query(query, context)?;

    let sql_query_string = sql_query.to_string();

    let formatted = format(
        &sql_query_string,
        &QueryParams::default(),
        FormatOptions::default(),
    );
    Ok(formatted)
}

pub fn translate_query(query: Query, context: Context) -> Result<sql_ast::Query> {
    // extract tables and the pipeline
    let tables = into_tables(query.nodes)?;

    let mut context = MaterializationContext::from(context);

    // split to atomics
    let atomics = atomic_tables_of_tables(tables, &mut context)?;

    // materialize each atomic in two stages
    let mut materialized = Vec::new();
    for t in atomics {
        let table_id = t.name.clone().and_then(|x| x.declared_at);

        let (pipeline, frame, c) = super::materialize(t.pipeline, context, table_id)?;
        context = c;

        materialized.push(AtomicTable {
            name: t.name,
            frame: Some(frame),
            pipeline,
        });
    }

    eprintln!("{context:?}");

    // take last table
    if materialized.is_empty() {
        bail!("No tables?");
    }
    let main_query = materialized.remove(materialized.len() - 1);
    let ctes = materialized;

    // convert each of the CTEs
    let ctes: Vec<_> = ctes
        .into_iter()
        .map(|t| table_to_sql_cte(t, &query.dialect))
        .try_collect()?;

    // convert main query
    let mut main_query = sql_query_of_atomic_table(main_query, &query.dialect)?;

    // attach CTEs
    if !ctes.is_empty() {
        main_query.with = Some(sql_ast::With {
            cte_tables: ctes,
            recursive: false,
        });
    }

    Ok(main_query)
}

pub struct AtomicTable {
    name: Option<TableRef>,
    pipeline: Pipeline,
    frame: Option<MaterializedFrame>,
}

fn into_tables(nodes: Vec<Node>) -> Result<Vec<Table>> {
    let mut tables: Vec<Table> = Vec::new();
    let mut pipeline: Vec<Node> = Vec::new();
    for node in nodes {
        match node.item {
            Item::Table(t) => tables.push(t),
            Item::Pipeline(p) => pipeline.extend(p.functions),
            i => bail!("Unexpected item on top level: {i:?}"),
        }
    }

    Ok([tables, vec![pipeline.into()]].concat())
}

fn table_to_sql_cte(table: AtomicTable, dialect: &Dialect) -> Result<sql_ast::Cte> {
    let alias = sql_ast::TableAlias {
        name: Item::Ident(table.name.clone().unwrap().name).try_into()?,
        columns: vec![],
    };
    Ok(sql_ast::Cte {
        alias,
        query: sql_query_of_atomic_table(table, dialect)?,
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

// impl Translator for
// fn sql_query_of_atomic_table(table: AtomicTable, dialect: &Dialect) -> Result<sql_ast::Query> {
fn sql_query_of_atomic_table(table: AtomicTable, dialect: &Dialect) -> Result<sql_ast::Query> {
    // TODO: possibly do validation here? e.g. check there isn't more than one
    // `from`? Or do we rely on the caller for that?

    let frame = table.frame.ok_or_else(|| anyhow!("frame not provided?"))?;

    let transforms = table.pipeline.into_transforms()?;

    let mut from = transforms
        .iter()
        .filter_map(|t| match t {
            Transform::From(table_ref) => Some(TableWithJoins {
                relation: table_factor_of_table_ref(table_ref),
                joins: vec![],
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    let joins = transforms
        .iter()
        .filter(|t| matches!(t, Transform::Join { .. }))
        .map(|t| match t {
            Transform::Join { side, with, filter } => {
                let constraint = match filter {
                    JoinFilter::On(nodes) => Item::Expr(nodes.to_vec())
                        .try_into()
                        .map(JoinConstraint::On)?,
                    JoinFilter::Using(nodes) => JoinConstraint::Using(
                        nodes
                            .iter()
                            .map(|x| x.item.clone().try_into())
                            .collect::<Result<Vec<_>>>()?,
                    ),
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
    let aggregate_position = transforms
        .iter()
        .position(|t| matches!(t, Transform::Aggregate { .. }))
        .unwrap_or(transforms.len());
    let (before, after) = transforms.split_at(aggregate_position);

    // Convert the filters in a pipeline into an Expr
    fn filter_of_pipeline(pipeline: &[Transform]) -> Result<Option<Expr>> {
        let filters: Vec<Vec<Node>> = pipeline
            .iter()
            .filter_map(|t| match t {
                Transform::Filter(filter) => Some(filter),
                _ => None,
            })
            .cloned()
            .collect();

        Ok(if !filters.is_empty() {
            Some((Item::Expr(combine_filters(filters))).try_into()?)
        } else {
            None
        })
    }
    // Find the filters that come before the aggregation.
    let where_ = filter_of_pipeline(before)?;
    let having = filter_of_pipeline(after)?;

    let take = transforms
        .iter()
        .filter_map(|t| match t {
            Transform::Take(take) => Some(*take),
            _ => None,
        })
        .min()
        .map(expr_of_i64);

    // If there is sort transform in the pipeline
    let sort = transforms.iter().any(|t| matches!(t, Transform::Sort(_)));
    let order_by = if sort {
        // Use sorting from the frame
        (frame.sort.iter())
            .map(|sort| OrderByExpr {
                expr: Item::Ident(sort.column.clone()).try_into().unwrap(),
                asc: if matches!(sort.direction, SortDirection::Asc) {
                    None // default order is ASC, so there is no need to emit it
                } else {
                    Some(false)
                },
                nulls_first: None,
            })
            .collect()
    } else {
        vec![]
    };

    let aggregate = transforms.get(aggregate_position);

    let group_bys: Vec<Node> = match aggregate {
        Some(Transform::Aggregate(select)) => select.group.clone(),
        None => vec![],
        _ => unreachable!("Expected an aggregate transformation"),
    };
    let group_by = Item::List(group_bys).try_into()?;

    let dialect = dialect.handler();

    Ok(sql_ast::Query {
        body: SetExpr::Select(Box::new(Select {
            distinct: false,
            top: if dialect.use_top() {
                take.clone().map(top_of_expr)
            } else {
                None
            },
            projection: (frame.columns.into_iter())
                .map(|n| n.item.try_into())
                .try_collect()?,
            from,
            lateral_views: vec![],
            selection: where_,
            group_by,
            cluster_by: vec![],
            distribute_by: vec![],
            sort_by: vec![],
            having,
        })),
        order_by,
        with: None,
        limit: if dialect.use_top() { None } else { take },
        offset: None,
        fetch: None,
    })
}

/// Convert a pipeline into a number of pipelines which can each "fit" into a SELECT.
fn atomic_pipelines_of_pipeline(pipeline: Pipeline) -> Result<Vec<AtomicTable>> {
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
    let pipeline = un_group::un_group(pipeline.functions)?;

    let mut counts: HashMap<&str, u32> = HashMap::new();
    let mut splits = vec![0];
    for (i, function) in pipeline.iter().enumerate() {
        let transform =
            (function.item.as_transform()).ok_or_else(|| anyhow!("expected Transform"))?;

        let split = match transform.as_ref() {
            "Join" => {
                counts.get("Filter").is_some()
                    || counts.get("Aggregate").is_some()
                    || counts.get("Sort").is_some()
                    || counts.get("Take").is_some()
            }
            "Aggregate" => {
                counts.get("Aggregate").is_some()
                    || counts.get("Sort").is_some()
                    || counts.get("Take").is_some()
            }
            "Filter" | "Sort" | "Take" => counts.get("Take").is_some(),
            _ => false,
        };

        if split {
            splits.push(i);
            counts.clear();
        }

        *counts.entry(transform.as_ref()).or_insert(0) += 1;
    }

    splits.push(pipeline.len());
    let ctes = (0..splits.len() - 1)
        .map(|i| pipeline[splits[i]..splits[i + 1]].to_vec())
        .filter(|x| !x.is_empty())
        .map(|p| p.into())
        .collect();
    Ok(ctes)
}

/// Converts a series of tables into a series of atomic tables, by putting the
/// next pipeline's `from` as the current pipelines's table name.
fn atomic_tables_of_tables(
    tables: Vec<Table>,
    context: &mut MaterializationContext,
) -> Result<Vec<AtomicTable>> {
    let mut atomics = Vec::new();
    let mut index = 0;
    for table in tables {
        // split table into atomics
        let pipeline = table.pipeline.item.into_pipeline()?;
        let mut t_atomics: Vec<_> = atomic_pipelines_of_pipeline(pipeline)?;

        let (last, ctes) = t_atomics
            .split_last_mut()
            .ok_or_else(|| anyhow!("No pipelines?"))?;

        // generate table names for all but last table
        let mut last_name = None;
        for cte in ctes {
            prepend_with_from(&mut cte.pipeline, last_name);

            let name = format!("table_{index}");
            let id = context.declare_table(&name);

            cte.name = Some(TableRef {
                name,
                alias: None,
                declared_at: Some(id),
            });
            index += 1;

            last_name = cte.name.clone();
        }

        // use original table name
        prepend_with_from(&mut last.pipeline, last_name);
        last.name = Some(TableRef {
            name: table.name,
            alias: None,
            declared_at: table.id,
        });

        atomics.extend(t_atomics);
    }
    Ok(atomics)
}

fn prepend_with_from(pipeline: &mut Pipeline, table: Option<TableRef>) {
    if let Some(table) = table {
        let from = Transform::From(table);
        pipeline.functions.insert(0, Item::Transform(from).into());
    }
}

/// Combines filters by putting them in parentheses and then joining them with `and`.
#[allow(unstable_name_collisions)] // Same behavior as the std lib; we can remove this + itertools when that's released.
fn combine_filters(filters: Vec<Vec<Node>>) -> Vec<Node> {
    filters
        .into_iter()
        .map(|f| Item::Expr(f).into())
        .intersperse(Item::Raw("and".to_owned()).into())
        .collect()
}

impl TryFrom<Item> for SelectItem {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        Ok(match item {
            Item::Expr(_) | Item::SString(_) | Item::FString(_) | Item::Ident(_) | Item::Raw(_) => {
                SelectItem::UnnamedExpr(Expr::try_from(item)?)
            }
            Item::NamedExpr(named) => SelectItem::ExprWithAlias {
                alias: sql_ast::Ident::new(named.name),
                expr: named.expr.item.try_into()?,
            },
            _ => bail!("Can't convert to SelectItem; {:?}", item),
        })
    }
}

fn expr_of_i64(number: i64) -> Expr {
    Expr::Value(Value::Number(
        number.to_string(),
        number.leading_zeros() < 32,
    ))
}

fn top_of_expr(take: Expr) -> Top {
    Top {
        quantity: Some(take),
        with_ties: false,
        percent: false,
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
                        .map(|node| TryInto::<Expr>::try_into(node.item))
                        .collect::<Result<Vec<Expr>>>()?
                        .iter()
                        .map(|x| x.to_string())
                        // Currently a hack, but maybe OK, since we don't
                        // need to parse every single expression into sqlparser ast.
                        .join(" "),
                ))
            }
            Item::Range(r) => {
                fn assert_bound(bound: Option<Box<Node>>) -> Result<Node, Error> {
                    bound.map(|b| *b).ok_or_else(|| {
                        Error::new(Reason::Simple(
                            "range requires both bounds to be used this way".to_string(),
                        ))
                    })
                }
                let start: Expr = assert_bound(r.start)?.item.try_into()?;
                let end: Expr = assert_bound(r.end)?.item.try_into()?;
                Expr::Identifier(sql_ast::Ident::new(format!("{} AND {}", start, end)))
            }
            Item::String(s) => Expr::Value(sql_ast::Value::SingleQuotedString(s)),
            // Fairly hacky — convert everything to a string, then concat it,
            // then convert to Expr. We can't use the `Item::Expr` code above
            // since we don't want to intersperse with spaces.
            Item::SString(s_string_items) => {
                let string = s_string_items
                    .into_iter()
                    .map(|s_string_item| match s_string_item {
                        InterpolateItem::String(string) => Ok(string),
                        InterpolateItem::Expr(node) => {
                            TryInto::<Expr>::try_into(node.item).map(|expr| expr.to_string())
                        }
                    })
                    .collect::<Result<Vec<String>>>()?
                    .join("");
                Item::Ident(string).try_into()?
            }
            Item::FString(f_string_items) => {
                let args = f_string_items
                    .into_iter()
                    .map(|item| match item {
                        InterpolateItem::String(string) => {
                            Ok(Expr::Value(sql_ast::Value::SingleQuotedString(string)))
                        }
                        InterpolateItem::Expr(node) => TryInto::<Expr>::try_into(node.item),
                    })
                    .map(|r| r.map(|e| FunctionArg::Unnamed(FunctionArgExpr::Expr(e))))
                    .collect::<Result<Vec<_>>>()?;

                Expr::Function(Function {
                    name: ObjectName(vec![sql_ast::Ident::new("CONCAT")]),
                    args,
                    distinct: false,
                    over: None,
                })
            }
            Item::Interval(interval) => {
                let sql_parser_datetime = match interval.unit.as_str() {
                    "years" => DateTimeField::Year,
                    "months" => DateTimeField::Month,
                    "days" => DateTimeField::Day,
                    "hours" => DateTimeField::Hour,
                    "minutes" => DateTimeField::Minute,
                    "seconds" => DateTimeField::Second,
                    _ => bail!("Unsupported interval unit: {}", interval.unit),
                };
                Expr::Value(Value::Interval {
                    value: interval.n.to_string(),
                    leading_field: Some(sql_parser_datetime),
                    leading_precision: None,
                    last_field: None,
                    fractional_seconds_precision: None,
                })
            }
            _ => bail!("Can't convert to Expr; {item:?}"),
        })
    }
}
impl TryFrom<Item> for Vec<Expr> {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::List(nodes) => Ok(nodes.into_iter().map(|x| x.item.try_into()).try_collect()?),
            _ => Err(anyhow!("Can't convert to Vec<Expr>; {item:?}")),
        }
    }
}
impl TryFrom<Item> for sql_ast::Ident {
    type Error = anyhow::Error;
    fn try_from(item: Item) -> Result<Self> {
        match item {
            Item::Ident(ident) => Ok(sql_ast::Ident::new(ident)),
            Item::Raw(ident) => Ok(sql_ast::Ident::new(ident)),
            _ => Err(anyhow!("Can't convert to Ident; {item:?}")),
        }
    }
}
impl From<Vec<Node>> for Table {
    fn from(functions: Vec<Node>) -> Self {
        Table {
            id: None,
            name: String::default(),
            pipeline: Box::new(Item::Pipeline(functions.into()).into()),
        }
    }
}
impl From<Vec<Node>> for AtomicTable {
    fn from(functions: Vec<Node>) -> Self {
        AtomicTable {
            name: None,
            pipeline: functions.into(),
            frame: None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{parser::parse, resolve, resolve_and_translate, sql::load_std_lib};
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
    fn test_f_string() {
        let query: Query = parse(
            r###"
        from employees
        derive age: year_born - s'now()'
        select [
            f"Hello my name is {prefix}{first_name} {last_name}",
            f"and I am {age} years old."
        ]
        "###,
        )
        .unwrap();

        let sql = resolve_and_translate(query).unwrap();
        assert_display_snapshot!(sql,
            @r###"
        SELECT
          CONCAT(
            'Hello my name is ',
            prefix,
            first_name,
            ' ',
            last_name
          ),
          CONCAT('and I am ', year_born - now(), ' years old.')
        FROM
          employees
        "###
        );
    }

    #[test]
    fn test_try_from_list_to_vec_expr() -> Result<()> {
        let item = Item::List(vec![
            Item::Ident("a".to_owned()).into(),
            Item::Ident("b".to_owned()).into(),
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

    fn parse_and_resolve(prql: &str) -> Result<Pipeline> {
        let std_lib = load_std_lib()?;
        let (_, context) = resolve(std_lib, None)?;

        let (mut nodes, _) = resolve(parse(prql)?.nodes, Some(context))?;
        let pipeline = nodes.remove(nodes.len() - 1);
        let pipeline = pipeline.item.into_pipeline()?;
        Ok(pipeline)
    }

    #[test]
    fn test_ctes_of_pipeline() -> Result<()> {
        // One aggregate, take at the end
        let prql: &str = r###"
        from employees
        filter country = "USA"
        aggregate [sal: average salary]
        sort sal
        take 20
        "###;

        let pipeline = parse_and_resolve(prql)?;
        let queries = atomic_pipelines_of_pipeline(pipeline)?;
        assert_eq!(queries.len(), 1);

        // One aggregate, but take at the top
        let prql: &str = r###"
        from employees
        take 20
        filter country = "USA"
        aggregate [sal: average salary]
        sort sal
        "###;

        let pipeline = parse_and_resolve(prql)?;
        let queries = atomic_pipelines_of_pipeline(pipeline)?;
        assert_eq!(queries.len(), 2);

        // A take, then two aggregates
        let prql: &str = r###"
        from employees
        take 20
        filter country = "USA"
        aggregate [sal: average salary]
        aggregate [sal: average sal]
        sort sal
        "###;

        let pipeline = parse_and_resolve(prql)?;
        let queries = atomic_pipelines_of_pipeline(pipeline)?;
        assert_eq!(queries.len(), 3);

        // A take, then a select
        let prql: &str = r###"
        from employees
        take 20
        select first_name
        "###;

        let pipeline = parse_and_resolve(prql)?;
        let queries = atomic_pipelines_of_pipeline(pipeline)?;
        assert_eq!(queries.len(), 1);
        Ok(())
    }

    #[test]
    fn test_sql_of_ast_1() -> Result<()> {
        let query: Query = parse(
            r###"
        from employees
        filter country = "USA"
        group [title, country] (
            aggregate [average salary]
        )
        sort title
        take 20
        "###,
        )?;

        let sql = resolve_and_translate(query)?;
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
        Ok(())
    }

    #[test]
    fn test_sql_of_ast_2() -> Result<()> {
        let query: Query = parse(
            r###"
        from employees
        aggregate sum_salary: s"count({salary})"
        filter sum_salary > 100
        "###,
        )?;
        let sql = resolve_and_translate(query)?;
        assert_snapshot!(sql, @r###"
        SELECT
          count(salary) AS sum_salary
        FROM
          employees
        HAVING
          count(salary) > 100
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
        let sql = resolve_and_translate(query)?;
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
  gross_cost:   gross_salary + benefits_cost     # Variables can use other variables.
]
filter gross_cost > 0
group [title, country] (
    aggregate  [                                 # `by` are the columns to group by.
        average salary,                          # These are aggregation calcs run on each group.
        sum     salary,
        average gross_salary,
        sum     gross_salary,
        average gross_cost,
        sum_gross_cost: sum gross_cost,
        ct: count,
    ]
)
sort sum_gross_cost
filter ct > 200
take 20
"#,
        )?;

        let sql = resolve_and_translate(query)?;
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
            group country (
                aggregate [
                    average_country_salary: average salary
                ]
            )
        )
        from newest_employees
        join average_salaries [country]
        select [name, salary, average_country_salary]
        "#,
        )?;
        let sql = resolve_and_translate(query)?;
        assert_display_snapshot!(sql,
            @r###"
        WITH newest_employees AS (
          SELECT
            employees.*
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
          average_salaries.salary,
          average_salaries.average_country_salary
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
            group [title, country] (
                aggregate [
                    salary: average salary
                ]
            )
            group [title, country] (
                aggregate [
                    sum_gross_cost: average salary
                ]
            )
            sort sum_gross_cost
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        WITH table_0 AS (
          SELECT
            employees.*
          FROM
            employees
          LIMIT
            20
        ), table_1 AS (
          SELECT
            title,
            country,
            AVG(salary) AS salary
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
          AVG(salary) AS sum_gross_cost
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

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        WITH table_0 AS (
          SELECT
            employees.*
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
    fn test_table_names_between_splits() {
        let prql = r###"
        from employees
        join d:department [dept_no]
        take 10
        join s:salaries [emp_no]
        select [employees.emp_no, d.name, s.salary]
        "###;
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
        assert_display_snapshot!(result, @r###"
        WITH table_0 AS (
          SELECT
            employees.*,
            d.*,
            dept_no
          FROM
            employees
            JOIN department AS d USING(dept_no)
          LIMIT
            10
        )
        SELECT
          table_0.emp_no,
          table_0.name,
          s.salary
        FROM
          table_0
          JOIN salaries AS s USING(emp_no)
        "###);

        let prql = r###"
        from e:employees
        take 10
        join salaries [emp_no]
        select [e.*, salary]
        "###;
        let result = parse(prql).and_then(resolve_and_translate).unwrap();
        assert_display_snapshot!(result, @r###"
        WITH table_0 AS (
          SELECT
            e.*
          FROM
            employees AS e
          LIMIT
            10
        )
        SELECT
          table_0.*,
          salary
        FROM
          table_0
          JOIN salaries USING(emp_no)
        "###);
    }

    #[test]
    fn test_table_alias() -> Result<()> {
        // Alias on from
        let query: Query = parse(
            r###"
            from e: employees
            join salaries side:left [salaries.emp_no = e.emp_no]
            group [e.emp_no] (
                aggregate [
                    emp_salary: average salary
                ]
            )
            select [e.emp_no, emp_salary]
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
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

    #[test]
    fn test_dialects() -> Result<()> {
        // Generic
        let query: Query = parse(
            r###"
        prql dialect:generic
        from Employees
        select [FirstName]
        take 3
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        SELECT
          FirstName
        FROM
          Employees
        LIMIT
          3
        "###);

        // SQL server
        let query: Query = parse(
            r###"
        prql dialect:ms_sql_server
        from Employees
        select [FirstName]
        take 3
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        SELECT
          TOP (3) FirstName
        FROM
          Employees
        "###);
        Ok(())
    }

    #[test]
    fn test_sorts() -> Result<()> {
        let query: Query = parse(
            r###"
        from Employees
        sort [id]
        sort [age, desc:last_name, asc:first_name]
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        SELECT
          Employees.*
        FROM
          Employees
        ORDER BY
          age,
          last_name DESC,
          first_name
        "###);

        Ok(())
    }

    #[test]
    fn test_ranges() -> Result<()> {
        let query: Query = parse(
            r###"
        from employees
        filter (age | in 18..40)
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        SELECT
          employees.*
        FROM
          employees
        WHERE
          age BETWEEN 18
          AND 40
        "###);

        let query: Query = parse(
            r###"
        from employees
        filter (age | in ..40)
        "###,
        )?;

        assert!(resolve_and_translate(query).is_err());

        Ok(())
    }

    #[test]
    fn test_interval() -> Result<()> {
        let query: Query = parse(
            r###"
        from projects
        derive first_check_in: start + 10days
        "###,
        )?;

        assert_display_snapshot!((resolve_and_translate(query)?), @r###"
        SELECT
          projects.*,
          start + INTERVAL '10' DAY AS first_check_in
        FROM
          projects
        "###);

        Ok(())
    }
}
