//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
use itertools::Itertools;
use regex::Regex;
use sqlparser::ast::{
    self as sql_ast, Join, JoinConstraint, JoinOperator, Select, SelectItem, SetExpr, TableAlias,
    TableFactor, TableWithJoins,
};

use super::gen_expr::*;
use super::gen_projection::*;
use super::operators::translate_operator;
use super::pq::ast::{Cte, CteKind, RelationExpr, RelationExprKind, SqlRelation, SqlTransform};
use super::{Context, Dialect};
use crate::debug;
use crate::ir::pl::{JoinSide, Literal};
use crate::ir::rq::{CId, Expr, ExprKind, RelationLiteral, RelationalQuery};
use crate::utils::{BreakUp, Pluck};
use crate::{Error, Result, WithErrorInfo};
use prqlc_parser::generic::InterpolateItem;

type Transform = SqlTransform<RelationExpr, ()>;

pub fn translate_query(query: RelationalQuery, dialect: Option<Dialect>) -> Result<sql_ast::Query> {
    // compile from RQ to PQ
    let (pq_query, mut ctx) = super::pq::compile_query(query, dialect)?;

    debug::log_stage(debug::Stage::Sql(debug::StageSql::Main));
    let mut query = translate_relation(pq_query.main_relation, &mut ctx)?;

    if !pq_query.ctes.is_empty() {
        // attach CTEs
        let mut cte_tables = Vec::new();
        let mut recursive = false;
        for cte in pq_query.ctes {
            let (cte, rec) = translate_cte(cte, &mut ctx)?;
            cte_tables.push(cte);
            recursive = recursive || rec;
        }
        query.with = Some(sql_ast::With {
            recursive,
            cte_tables,
        });
    }

    debug::log_entry(|| debug::DebugEntryKind::ReprSqlParser(query.clone()));
    Ok(query)
}

fn translate_relation(relation: SqlRelation, ctx: &mut Context) -> Result<sql_ast::Query> {
    match relation {
        SqlRelation::AtomicPipeline(pipeline) => translate_pipeline(pipeline, ctx),
        SqlRelation::Literal(data) => translate_relation_literal(data, ctx),
        SqlRelation::SString(items) => translate_query_sstring(items, ctx),
        SqlRelation::Operator { name, args } => translate_query_operator(name, args, ctx),
    }
}

fn translate_pipeline(pipeline: Vec<Transform>, ctx: &mut Context) -> Result<sql_ast::Query> {
    use SqlTransform::*;

    let (select, set_ops) =
        pipeline.break_up(|t| matches!(t, Union { .. } | Except { .. } | Intersect { .. }));

    let select = translate_select_pipeline(select, ctx)?;

    translate_set_ops_pipeline(select, set_ops, ctx)
}

fn translate_select_pipeline(
    mut pipeline: Vec<Transform>,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    let table_count = count_tables(&pipeline);
    log::debug!("atomic query contains {table_count} tables");
    ctx.push_query();
    ctx.query.omit_ident_prefix = table_count == 1;
    ctx.query.pre_projection = true;

    let mut from: Vec<_> = pipeline
        .pluck(|t| t.into_from())
        .into_iter()
        .map(|source| -> Result<TableWithJoins> {
            Ok(TableWithJoins {
                relation: translate_relation_expr(source, ctx)?,
                joins: vec![],
            })
        })
        .try_collect()?;

    let joins = pipeline
        .pluck(|t| t.into_join())
        .into_iter()
        .map(|j| translate_join(j, ctx))
        .collect::<Result<Vec<_>>>()?;
    if !joins.is_empty() {
        if let Some(from) = from.last_mut() {
            from.joins = joins;
        } else {
            unreachable!()
        }
    }

    let projection = pipeline
        .pluck(|t| t.into_select())
        .into_iter()
        .exactly_one()
        .unwrap();
    let projection = translate_wildcards(&ctx.anchor, projection);
    let projection = translate_select_items(projection.0, projection.1, ctx)?;

    let order_by = pipeline.pluck(|t| t.into_sort());
    let takes = pipeline.pluck(|t| t.into_take());
    let is_distinct = pipeline.iter().any(|t| matches!(t, SqlTransform::Distinct));
    let distinct_ons = pipeline.pluck(|t| t.into_distinct_on());
    let distinct = if is_distinct {
        Some(sql_ast::Distinct::Distinct)
    } else if !distinct_ons.is_empty() {
        Some(sql_ast::Distinct::On(
            distinct_ons
                .into_iter()
                .exactly_one()
                .unwrap()
                .into_iter()
                .map(|id| translate_cid(id, ctx).map(|x| x.into_ast()))
                .collect::<Result<Vec<_>>>()?,
        ))
    } else {
        None
    };

    // Split the pipeline into before & after the aggregate
    let (mut before_agg, mut after_agg) =
        pipeline.break_up(|t| matches!(t, Transform::Aggregate { .. } | Transform::Union { .. }));

    // WHERE and HAVING
    let where_ = filter_of_conditions(before_agg.pluck(|t| t.into_filter()), ctx)?;
    let having = filter_of_conditions(after_agg.pluck(|t| t.into_filter()), ctx)?;

    // GROUP BY
    let aggregate = after_agg.pluck(|t| t.into_aggregate()).into_iter().next();
    let group_by: Vec<CId> = aggregate.map(|(part, _)| part).unwrap_or_default();
    ctx.query.allow_stars = ctx.dialect.stars_in_group();
    let group_by = sql_ast::GroupByExpr::Expressions(try_into_exprs(group_by, ctx, None)?);
    ctx.query.allow_stars = true;

    ctx.query.pre_projection = false;

    let ranges = takes.into_iter().map(|x| x.range).collect();
    let take = range_of_ranges(ranges)?;
    let offset = take.start.map(|s| s - 1).unwrap_or(0);
    let limit = take.end.map(|e| e - offset);

    let mut offset = if offset == 0 {
        None
    } else {
        let kind = ExprKind::Literal(Literal::Integer(offset));
        let expr = Expr { kind, span: None };
        Some(sqlparser::ast::Offset {
            value: translate_expr(expr, ctx)?.into_ast(),
            rows: if ctx.dialect.use_fetch() {
                sqlparser::ast::OffsetRows::Rows
            } else {
                sqlparser::ast::OffsetRows::None
            },
        })
    };

    // Use sorting from the frame
    let mut order_by: Vec<sql_ast::OrderByExpr> = order_by
        .last()
        .map(|sorts| {
            sorts
                .iter()
                .map(|s| translate_column_sort(s, ctx))
                .try_collect()
        })
        .transpose()?
        .unwrap_or_default();

    let (fetch, limit) = if ctx.dialect.use_fetch() {
        (limit.map(|l| fetch_of_i64(l, ctx)), None)
    } else {
        (None, limit.map(expr_of_i64))
    };

    // If we have a FETCH we need to make sure that:
    // - we have an OFFSET (set to 0)
    // - we have an ORDER BY (see https://stackoverflow.com/a/44919325)
    if fetch.is_some() {
        if offset.is_none() {
            let kind = ExprKind::Literal(Literal::Integer(0));
            let expr = Expr { kind, span: None };
            offset = Some(sqlparser::ast::Offset {
                value: translate_expr(expr, ctx)?.into_ast(),
                rows: sqlparser::ast::OffsetRows::Rows,
            })
        }
        if order_by.is_empty() {
            order_by.push(sql_ast::OrderByExpr {
                expr: sql_ast::Expr::Value(sql_ast::Value::Placeholder(
                    "(SELECT NULL)".to_string(),
                )),
                asc: None,
                nulls_first: None,
            });
        }
    }

    ctx.pop_query();

    Ok(sql_ast::Query {
        order_by,
        limit,
        offset,
        fetch,
        ..default_query(SetExpr::Select(Box::new(Select {
            distinct,
            projection,
            from,
            selection: where_,
            group_by,
            having,
            ..default_select()
        })))
    })
}

fn translate_set_ops_pipeline(
    mut top: sql_ast::Query,
    mut pipeline: Vec<Transform>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    // reverse, so it's easier (and O(1)) to pop
    pipeline.reverse();

    while let Some(transform) = pipeline.pop() {
        use SqlTransform::*;

        let op = match &transform {
            Union { .. } => sql_ast::SetOperator::Union,
            Except { .. } => sql_ast::SetOperator::Except,
            Intersect { .. } => sql_ast::SetOperator::Intersect,
            Sort(_) => continue,
            _ => unreachable!(),
        };

        let (distinct, bottom) = match transform {
            Union { distinct, bottom }
            | Except { distinct, bottom }
            | Intersect { distinct, bottom } => (distinct, bottom),
            _ => unreachable!(),
        };

        // prepare top
        let left = query_to_set_expr(top, context);

        top = default_query(SetExpr::SetOperation {
            left,
            right: Box::new(SetExpr::Select(Box::new(sql_ast::Select {
                projection: vec![SelectItem::Wildcard(
                    sql_ast::WildcardAdditionalOptions::default(),
                )],
                from: vec![TableWithJoins {
                    relation: translate_relation_expr(bottom, context)?,
                    joins: vec![],
                }],
                ..default_select()
            }))),
            set_quantifier: if distinct {
                if context.dialect.set_ops_distinct() {
                    sql_ast::SetQuantifier::Distinct
                } else {
                    sql_ast::SetQuantifier::None
                }
            } else {
                sql_ast::SetQuantifier::All
            },
            op,
        });
    }

    Ok(top)
}

fn translate_relation_expr(relation_expr: RelationExpr, ctx: &mut Context) -> Result<TableFactor> {
    let alias = Some(&relation_expr.riid)
        .and_then(|riid| ctx.anchor.relation_instances.get(riid))
        .and_then(|ri| ri.table_ref.name.clone());

    Ok(match relation_expr.kind {
        RelationExprKind::Ref(tid) => {
            let decl = ctx.anchor.lookup_table_decl(&tid).unwrap();

            // prepare names
            let table_name = decl.name.clone().unwrap();

            let name = sql_ast::ObjectName(translate_ident(Some(table_name.clone()), None, ctx));

            TableFactor::Table {
                name,
                alias: if Some(table_name.name) == alias {
                    None
                } else {
                    translate_table_alias(alias, ctx)
                },
                args: None,
                with_hints: vec![],
                version: None,
                partitions: vec![],
            }
        }
        RelationExprKind::SubQuery(query) => {
            let query = translate_relation(query, ctx)?;

            let alias = translate_table_alias(alias, ctx);

            TableFactor::Derived {
                lateral: false,
                subquery: Box::new(query),
                alias,
            }
        }
    })
}

fn translate_table_alias(alias: Option<String>, ctx: &mut Context) -> Option<TableAlias> {
    alias
        .map(|ident| translate_ident_part(ident, ctx))
        .map(simple_table_alias)
}

fn translate_join(
    (side, with, filter): (JoinSide, RelationExpr, Expr),
    ctx: &mut Context,
) -> Result<Join> {
    let relation = translate_relation_expr(with, ctx)?;

    let constraint = JoinConstraint::On(translate_expr(filter, ctx)?.into_ast());

    Ok(Join {
        relation,
        join_operator: match side {
            JoinSide::Inner => JoinOperator::Inner(constraint),
            JoinSide::Left => JoinOperator::LeftOuter(constraint),
            JoinSide::Right => JoinOperator::RightOuter(constraint),
            JoinSide::Full => JoinOperator::FullOuter(constraint),
        },
    })
}

fn translate_cte(cte: Cte, ctx: &mut Context) -> Result<(sql_ast::Cte, bool)> {
    let decl = ctx.anchor.lookup_table_decl(&cte.tid).unwrap();
    let cte_name = decl.name.clone().unwrap();

    let cte_name = translate_ident(Some(cte_name), None, ctx).pop().unwrap();

    let (query, recursive) = match cte.kind {
        // base case
        CteKind::Normal(rel) => (translate_relation(rel, ctx)?, false),

        // special: WITH RECURSIVE
        CteKind::Loop { initial, step } => {
            // compile initial
            let initial = query_to_set_expr(translate_relation(initial, ctx)?, ctx);

            let step = query_to_set_expr(translate_relation(step, ctx)?, ctx);

            // build CTE and its SELECT
            let inner_query = default_query(SetExpr::SetOperation {
                op: sql_ast::SetOperator::Union,
                set_quantifier: sql_ast::SetQuantifier::All,
                left: initial,
                right: step,
            });

            (inner_query, true)

            // RECURSIVE can only follow WITH directly.
            // Initial implementation assumed that it applies only to the first CTE.
            // This meant that it had to wrap any-non-first CTE into a *nested* WITH, so the inner
            // WITH could be RECURSIVE.
            // This is implementation of that, in case some dialect requires it.
            // let inner_cte = sql_ast::Cte {
            //     alias: simple_table_alias(cte_name.clone()),
            //     query: Box::new(inner_query),
            //     from: None,
            // };
            // let outer_query = sql_ast::Query {
            //     with: Some(sql_ast::With {
            //         recursive: true,
            //         cte_tables: vec![inner_cte],
            //     }),
            //     ..default_query(sql_ast::SetExpr::Select(Box::new(sql_ast::Select {
            //         projection: vec![SelectItem::Wildcard(
            //             sql_ast::WildcardAdditionalOptions::default(),
            //         )],
            //         from: vec![TableWithJoins {
            //             relation: TableFactor::Table {
            //                 name: sql_ast::ObjectName(vec![cte_name.clone()]),
            //                 alias: None,
            //                 args: None,
            //                 with_hints: Vec::new(),
            //             },
            //             joins: vec![],
            //         }],
            //         ..default_select()
            //     })))
            // };
            // (outer_query, false)
        }
    };

    let cte = sql_ast::Cte {
        alias: simple_table_alias(cte_name),
        query: Box::new(query),
        from: None,
        materialized: None,
    };
    Ok((cte, recursive))
}

fn translate_relation_literal(data: RelationLiteral, ctx: &Context) -> Result<sql_ast::Query> {
    // TODO: this could be made to use VALUES instead of SELECT UNION ALL SELECT
    //       I'm not sure about compatibility though.

    let mut selects = Vec::with_capacity(data.rows.len());

    for row in data.rows {
        let body = sql_ast::SetExpr::Select(Box::new(Select {
            projection: std::iter::zip(data.columns.clone(), row)
                .map(|(col, value)| -> Result<_> {
                    Ok(SelectItem::ExprWithAlias {
                        expr: translate_literal(value, ctx)?,
                        alias: translate_ident_part(col, ctx),
                    })
                })
                .try_collect()?,
            ..default_select()
        }));

        selects.push(body)
    }

    if selects.is_empty() {
        return Err(
            Error::new_simple("No rows provided for `from_text`".to_string()).push_hint(
                "add a newline, then a row of data following the column. If using \
                the json format, ensure `data` isn't empty",
            ),
        );
    }

    let mut body = selects.remove(0);
    for select in selects {
        body = SetExpr::SetOperation {
            op: sql_ast::SetOperator::Union,
            set_quantifier: sql_ast::SetQuantifier::All,
            left: Box::new(body),
            right: Box::new(select),
        }
    }

    Ok(default_query(body))
}

pub(super) fn translate_query_sstring(
    items: Vec<InterpolateItem<Expr>>,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    let string = translate_sstring(items, ctx)?;

    let re = Regex::new(r"(?i)^SELECT\b").unwrap();
    let prefix = if let Some(string) = string.trim().get(0..7) {
        string
    } else {
        ""
    };

    if re.is_match(prefix) {
        if let Some(string) = string.trim().strip_prefix(prefix) {
            return Ok(default_query(sql_ast::SetExpr::Select(Box::new(
                sql_ast::Select {
                    projection: vec![sql_ast::SelectItem::UnnamedExpr(sql_ast::Expr::Identifier(
                        sql_ast::Ident::new(string),
                    ))],
                    ..default_select()
                },
            ))));
        }
    }

    Err(
        Error::new_simple("s-strings representing a table must start with `SELECT `".to_string())
            .push_hint("this is a limitation by current compiler implementation"),
    )
}

pub(super) fn translate_query_operator(
    name: String,
    args: Vec<Expr>,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    let from_s_string = translate_operator(name, args, ctx)?;

    let s_string = format!(" * FROM {}", from_s_string.text);

    Ok(default_query(sql_ast::SetExpr::Select(Box::new(
        sql_ast::Select {
            projection: vec![sql_ast::SelectItem::UnnamedExpr(sql_ast::Expr::Identifier(
                sql_ast::Ident::new(s_string),
            ))],
            ..default_select()
        },
    ))))
}

fn filter_of_conditions(exprs: Vec<Expr>, context: &mut Context) -> Result<Option<sql_ast::Expr>> {
    Ok(if let Some(cond) = all(exprs) {
        Some(translate_expr(cond, context)?.into_ast())
    } else {
        None
    })
}

fn all(mut exprs: Vec<Expr>) -> Option<Expr> {
    let mut condition = exprs.pop()?;
    while let Some(expr) = exprs.pop() {
        condition = Expr {
            kind: ExprKind::Operator {
                name: "std.and".to_string(),
                args: vec![expr, condition],
            },
            span: None,
        };
    }
    Some(condition)
}

fn default_query(body: sql_ast::SetExpr) -> sql_ast::Query {
    sql_ast::Query {
        with: None,
        body: Box::new(body),
        order_by: Vec::new(),
        limit: None,
        offset: None,
        fetch: None,
        locks: Vec::new(),
        limit_by: Vec::new(),
        for_clause: None,
    }
}

fn default_select() -> Select {
    Select {
        distinct: None,
        top: None,
        projection: Vec::new(),
        into: None,
        from: Vec::new(),
        lateral_views: Vec::new(),
        selection: None,
        group_by: sql_ast::GroupByExpr::Expressions(vec![]),
        cluster_by: Vec::new(),
        distribute_by: Vec::new(),
        sort_by: Vec::new(),
        having: None,
        named_window: vec![],
        qualify: None,
        value_table_mode: None,
        window_before_qualify: false,
        connect_by: None,
    }
}

fn simple_table_alias(name: sql_ast::Ident) -> TableAlias {
    TableAlias {
        name,
        columns: Vec::new(),
    }
}

fn query_to_set_expr(query: sql_ast::Query, context: &mut Context) -> Box<SetExpr> {
    let is_simple = query.with.is_none()
        && query.order_by.is_empty()
        && query.limit.is_none()
        && query.offset.is_none()
        && query.fetch.is_none()
        && query.locks.is_empty();

    if is_simple {
        return query.body;
    }

    // query is not simple, so we need to wrap it into
    // `SELECT * FROM (query)`
    Box::new(SetExpr::Select(Box::new(Select {
        projection: vec![SelectItem::Wildcard(
            sql_ast::WildcardAdditionalOptions::default(),
        )],
        from: vec![TableWithJoins {
            relation: TableFactor::Derived {
                lateral: false,
                subquery: Box::new(query),
                alias: Some(simple_table_alias(sql_ast::Ident::new(
                    context.anchor.table_name.gen(),
                ))),
            },
            joins: vec![],
        }],
        ..default_select()
    })))
}

fn count_tables(transforms: &[Transform]) -> usize {
    let mut count = 0;
    for transform in transforms {
        if let Transform::Join { .. } | Transform::From(_) = transform {
            count += 1;
        }
    }

    count
}
#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    #[test]
    fn test_variable_after_aggregate() {
        let query = &r#"
        from employees
        group {title, emp_no} (
            aggregate {emp_salary = average salary}
        )
        group {title} (
            aggregate {avg_salary = average emp_salary}
        )
        "#;

        let sql_ast = crate::tests::compile(query).unwrap();

        assert_snapshot!(sql_ast, @r###"
        WITH table_0 AS (
          SELECT
            title,
            AVG(salary) AS _expr_0
          FROM
            employees
          GROUP BY
            title,
            emp_no
        )
        SELECT
          title,
          AVG(_expr_0) AS avg_salary
        FROM
          table_0
        GROUP BY
          title
        "###);
    }

    #[test]
    fn test_derive_filter() {
        // I suspect that the anchoring algorithm has a architectural flaw:
        // it assumes that it can materialize all columns, even if their
        // Compute is in a prior CTE. The problem is that because anchoring is
        // computed back-to-front, we don't know where Compute will end up when
        // materializing following transforms.
        //
        // If algorithm is changed to be front-to-back, preprocess_reorder can
        // be (must be) removed.

        let query = &r#"
        from employees
        derive {global_rank = rank country}
        filter country == "USA"
        derive {rank = rank country}
        "#;

        let sql_ast = crate::tests::compile(query).unwrap();

        assert_snapshot!(sql_ast, @r###"
        WITH table_0 AS (
          SELECT
            *,
            RANK() OVER () AS global_rank
          FROM
            employees
        )
        SELECT
          *,
          RANK() OVER () AS rank
        FROM
          table_0
        WHERE
          country = 'USA'
        "###);
    }

    #[test]
    fn test_filter_windowed() {
        // #806
        let query = &r#"
        from tbl1
        filter (average bar) > 3
        "#;

        assert_snapshot!(crate::tests::compile(query).unwrap(), @r###"
        WITH table_0 AS (
          SELECT
            *,
            AVG(bar) OVER () AS _expr_0
          FROM
            tbl1
        )
        SELECT
          *
        FROM
          table_0
        WHERE
          _expr_0 > 3
        "###);
    }
}
