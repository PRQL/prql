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
use sqlparser::ast::{self as sql_ast, Select, SetExpr, TableWithJoins};
use std::collections::HashMap;

use crate::ast::{DialectHandler, Literal};
use crate::ir::{CId, Expr, ExprKind, Query, Table, Transform};

use super::anchor::AnchorContext;
use super::codegen::*;

pub(super) struct Context {
    pub dialect: Box<dyn DialectHandler>,
    pub anchor: AnchorContext,
}

/// Translate a PRQL AST into a SQL string.
pub fn translate(query: Query) -> Result<String> {
    let sql_query = translate_query(query)?;

    let sql_query_string = sql_query.to_string();

    let formatted = format(
        &sql_query_string,
        &QueryParams::default(),
        FormatOptions::default(),
    );

    // The sql formatter turns `{{` into `{ {`, and while that's reasonable SQL,
    // we want to allow jinja expressions through. So we (somewhat hackily) replace
    // any `{ {` with `{{`.
    let formatted = formatted.replace("{ {", "{{").replace("} }", "}}");

    Ok(formatted)
}

pub fn translate_query(query: Query) -> Result<sql_ast::Query> {
    let dialect = query.def.clone().dialect.handler();

    let (anchor, query) = AnchorContext::of(query);

    let mut context = Context { dialect, anchor };

    // extract tables and the pipeline
    let tables = into_tables(query.main_pipeline, query.tables, &mut context)?;

    // split to atomics
    let mut atomics = atomic_queries_of_tables(tables, &mut context);

    // take last table
    if atomics.is_empty() {
        bail!("No tables?");
    }
    let main_query = atomics.remove(atomics.len() - 1);
    let ctes = atomics;

    // convert each of the CTEs
    let ctes: Vec<_> = ctes
        .into_iter()
        .map(|t| table_to_sql_cte(t, &mut context))
        .try_collect()?;

    // convert main query
    let mut main_query = sql_query_of_atomic_query(main_query.pipeline, &mut context)?;

    // attach CTEs
    if !ctes.is_empty() {
        main_query.with = Some(sql_ast::With {
            cte_tables: ctes,
            recursive: false,
        });
    }

    Ok(main_query)
}

/// A query that can be expressed with one SELECT statement
#[derive(Debug)]
pub struct AtomicQuery {
    name: Option<String>,
    pipeline: Vec<Transform>,
}

fn into_tables(
    main_pipeline: Vec<Transform>,
    tables: Vec<Table>,
    context: &mut Context,
) -> Result<Vec<Table>> {
    let main = Table {
        id: context.anchor.ids.gen_tid(),
        name: None,
        pipeline: main_pipeline,
    };
    Ok([tables, vec![main]].concat())
}

fn table_to_sql_cte(table: AtomicQuery, context: &mut Context) -> Result<sql_ast::Cte> {
    let alias = sql_ast::TableAlias {
        name: translate_ident_part(table.name.unwrap(), context),
        columns: vec![],
    };
    Ok(sql_ast::Cte {
        alias,
        query: sql_query_of_atomic_query(table.pipeline, context)?,
        from: None,
    })
}

fn sql_query_of_atomic_query(
    pipeline: Vec<Transform>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    let select = determine_select_columns(&pipeline);

    let mut from = pipeline
        .iter()
        .filter_map(|t| match &t {
            Transform::From(table_ref, _) => Some(TableWithJoins {
                relation: table_factor_of_table_ref(table_ref, context),
                joins: vec![],
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    let joins = pipeline
        .iter()
        .filter(|t| matches!(t, Transform::Join { .. }))
        .map(|j| translate_join(&j, context))
        .collect::<Result<Vec<_>>>()?;
    if !joins.is_empty() {
        if let Some(from) = from.last_mut() {
            from.joins = joins;
        } else {
            return Err(anyhow!("Cannot use `join` without `from`"));
        }
    }

    // Split the pipeline into before & after the aggregate
    let aggregate_position = pipeline
        .iter()
        .position(|t| matches!(t, Transform::Aggregate { .. }))
        .unwrap_or(pipeline.len());
    let (before, after) = pipeline.split_at(aggregate_position);

    // Find the filters that come before the aggregation.
    let where_ = filter_of_pipeline(before, context)?;
    let having = filter_of_pipeline(after, context)?;

    let takes = pipeline
        .iter()
        .filter_map(|t| match &t {
            Transform::Take(range) => Some(range.clone()),
            _ => None,
        })
        .collect();
    let take = range_of_ranges(takes)?;
    let offset = take.start.map(|s| s - 1).unwrap_or(0);
    let limit = take.end.map(|e| e - offset);

    let offset = if offset == 0 {
        None
    } else {
        Some(sqlparser::ast::Offset {
            value: translate_expr_kind(ExprKind::Literal(Literal::Integer(offset)), context)?,
            rows: sqlparser::ast::OffsetRows::None,
        })
    };

    // Use sorting from the frame
    let order_by = pipeline
        .iter()
        .filter_map(|t| match t {
            Transform::Sort(cols) => Some(cols),
            _ => None,
        })
        .last()
        .iter()
        .flat_map(|sorts| sorts.iter().map(|s| translate_column_sort(s, context)))
        .try_collect()?;

    let aggregate = pipeline.get(aggregate_position);

    let group_bys: Vec<Expr> = match aggregate {
        Some(Transform::Aggregate(_)) => vec![], // TODO: add by argument to Aggregate and use it here
        None => vec![],
        _ => unreachable!("Expected an aggregate transformation"),
    };

    let distinct = pipeline.iter().any(|t| matches!(t, Transform::Unique));

    Ok(sql_ast::Query {
        body: Box::new(SetExpr::Select(Box::new(Select {
            distinct,
            top: if context.dialect.use_top() {
                limit.map(|l| top_of_i64(l, context))
            } else {
                None
            },
            projection: select
                .into_iter()
                .map(|id| translate_select_item(id, context))
                .try_collect()?,
            into: None,
            from,
            lateral_views: vec![],
            selection: where_,
            group_by: try_into_exprs(group_bys, context)?,
            cluster_by: vec![],
            distribute_by: vec![],
            sort_by: vec![],
            having,
            qualify: None,
        }))),
        order_by,
        with: None,
        limit: if context.dialect.use_top() {
            None
        } else {
            limit.map(expr_of_i64)
        },
        offset,
        fetch: None,
        lock: None,
    })
}

pub(super) fn determine_select_columns(pipeline: &[Transform]) -> Vec<CId> {
    let mut columns = Vec::new();

    for transform in pipeline {
        match transform {
            Transform::From(_, col_defs) => columns = col_defs.iter().map(|def| def.id).collect(),
            Transform::Select(cols) => columns = cols.clone(),
            _ => {}
        }
    }

    columns
}

/// Finds maximum number of transforms that can "fit" into a SELECT query.
fn find_next_atomic_split(pipeline: &[Transform]) -> Option<usize> {
    // Pipeline must be split when there is a transformation that out of order:
    // - joins (no limit),
    // - filters (for WHERE)
    // - aggregate (max 1x)
    // - sort (no limit)
    // - filters (for HAVING)
    // - take (no limit)

    let mut counts: HashMap<&str, u32> = HashMap::new();
    for (i, transform) in pipeline.iter().enumerate() {
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
            "Sort" => counts.get("Take").is_some(),
            "Filter" => counts.get("Take").is_some(), // || transform.is_complex,

            // There can be many takes, but they have to be consecutive
            // For example `take 100 | sort a | take 10` can't be one CTE.
            // But this is enforced by transform order anyway.
            "Take" => false,

            _ => false,
        };

        if split {
            return Some(i);
        }

        *counts.entry(transform.as_ref()).or_insert(0) += 1;
    }

    return None;
}

/// Converts a series of tables into a series of atomic tables, by putting the
/// next pipeline's `from` as the current pipelines's table name.
fn atomic_queries_of_tables(tables: Vec<Table>, context: &mut Context) -> Vec<AtomicQuery> {
    tables
        .into_iter()
        .flat_map(|t| atomic_queries_of_table(t, context))
        .collect()
}

fn atomic_queries_of_table(table: Table, context: &mut Context) -> Vec<AtomicQuery> {
    let mut atomics = Vec::new();

    let mut pipeline = table.pipeline;
    while let Some(at_position) = find_next_atomic_split(&pipeline) {
        let name = context.anchor.gen_table_name();

        let split = context.anchor.split_pipeline(pipeline, at_position, &name);
        atomics.push(AtomicQuery {
            name: Some(name),
            pipeline: split.0,
        });
        pipeline = split.1;
    }

    atomics.push(AtomicQuery {
        name: table.name,
        pipeline,
    });

    atomics
}

fn filter_of_pipeline(pipeline: &[Transform], context: &Context) -> Result<Option<sql_ast::Expr>> {
    let filters: Vec<Expr> = pipeline
        .iter()
        .filter_map(|t| match &t {
            Transform::Filter(filter) => Some(filter.clone()),
            _ => None,
        })
        .collect();
    filter_of_filters(filters, context)
}

impl From<Vec<Transform>> for AtomicQuery {
    fn from(pipeline: Vec<Transform>) -> Self {
        AtomicQuery {
            name: None,
            pipeline,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{ast::GenericDialect, parse, semantic::resolve};

    fn parse_and_resolve(prql: &str) -> Result<(Table, Context)> {
        let query = resolve(parse(prql)?, None)?.0;
        let (anchor, query) = AnchorContext::of(query);
        let mut context = Context {
            dialect: Box::new(GenericDialect {}),
            anchor,
        };

        let table = Table {
            id: context.anchor.ids.gen_tid(),
            name: None,
            pipeline: query.main_pipeline,
        };
        Ok((table, context))
    }

    #[test]
    fn test_ctes_of_pipeline() {
        // One aggregate, take at the end
        let prql: &str = r###"
        from employees
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        take 20
        "###;

        let (table, mut context) = parse_and_resolve(prql).unwrap();
        let queries = atomic_queries_of_table(table, &mut context);
        assert_eq!(queries.len(), 1);

        // One aggregate, but take at the top
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        "###;

        let (table, mut context) = parse_and_resolve(prql).unwrap();
        let queries = atomic_queries_of_table(table, &mut context);
        assert_eq!(queries.len(), 2);

        // A take, then two aggregates
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        aggregate [sal = average sal]
        sort sal
        "###;

        let (table, mut context) = parse_and_resolve(prql).unwrap();
        let queries = atomic_queries_of_table(table, &mut context);
        assert_eq!(queries.len(), 3);

        // A take, then a select
        let prql: &str = r###"
        from employees
        take 20
        select first_name
        "###;

        let (table, mut context) = parse_and_resolve(prql).unwrap();
        let queries = atomic_queries_of_table(table, &mut context);
        assert_eq!(queries.len(), 1);
    }
}
