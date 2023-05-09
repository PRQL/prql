//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
use anyhow::{anyhow, Result};
use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, Ident, Join, JoinConstraint, JoinOperator, Select, SelectItem, SetExpr,
    TableAlias, TableFactor, TableWithJoins,
};

use crate::ast::pl::{JoinSide, Literal, RelationLiteral};
use crate::ast::rq::{CId, Expr, ExprKind, Query, TId};
use crate::utils::{BreakUp, Pluck};

use super::gen_expr::*;
use super::gen_projection::*;
use super::srq::ast::{RelationExpr, SqlRelation, SqlTransform};

use super::{Context, Dialect};

type Transform = SqlTransform<RelationExpr, ()>;

pub fn translate_query(query: Query, dialect: Option<Dialect>) -> Result<sql_ast::Query> {
    // compile from RQ to SRQ
    let (srq_query, mut ctx) = super::srq::compile_query(query, dialect)?;

    let mut query = translate_relation(srq_query.main_relation, &mut ctx)?;

    if !srq_query.ctes.is_empty() {
        // attach CTEs
        let cte_tables = srq_query
            .ctes
            .into_iter()
            .map(|(tid, rel)| translate_cte(tid, rel, &mut ctx))
            .try_collect()?;
        query.with = Some(sql_ast::With {
            recursive: false,
            cte_tables,
        });
    }

    Ok(query)
}

fn translate_cte(tid: TId, rel: SqlRelation, ctx: &mut Context) -> Result<sql_ast::Cte> {
    let query = translate_relation(rel, ctx)?;

    let decl = ctx.anchor.table_decls.get_mut(&tid).unwrap();
    let table_name = decl.name.clone().unwrap_or_else(|| {
        let n = ctx.anchor.table_name.gen();
        decl.name = Some(n.clone());
        n
    });

    let table_name = translate_ident_part(table_name, ctx);

    Ok(sql_ast::Cte {
        alias: simple_table_alias(table_name),
        query: Box::new(query),
        from: None,
    })
}

fn translate_relation(relation: SqlRelation, ctx: &mut Context) -> Result<sql_ast::Query> {
    match relation {
        SqlRelation::AtomicPipeline(pipeline) => translate_pipeline(pipeline, ctx),
        SqlRelation::Literal(data) => translate_relation_literal(data, ctx),
        SqlRelation::SString(items) => translate_query_sstring(items, ctx),
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
            return Err(anyhow!("Cannot use `join` without `from`"));
        }
    }

    let projection = pipeline
        .pluck(|t| t.into_select())
        .into_iter()
        .exactly_one()
        .unwrap();
    let projection = translate_wildcards(&ctx.anchor, projection);
    let projection = translate_select_items(projection.0, projection.1, ctx)?;

    let sorts = pipeline.pluck(|t| t.into_sort());
    let takes = pipeline.pluck(|t| t.into_take());
    let distinct = pipeline.iter().any(|t| matches!(t, SqlTransform::Distinct));

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
    let group_by = try_into_exprs(group_by, ctx, None)?;
    ctx.query.allow_stars = true;

    ctx.query.pre_projection = false;

    let ranges = takes.into_iter().map(|x| x.range).collect();
    let take = range_of_ranges(ranges)?;
    let offset = take.start.map(|s| s - 1).unwrap_or(0);
    let limit = take.end.map(|e| e - offset);

    let offset = if offset == 0 {
        None
    } else {
        let kind = ExprKind::Literal(Literal::Integer(offset));
        let expr = Expr { kind, span: None };
        Some(sqlparser::ast::Offset {
            value: translate_expr(expr, ctx)?,
            rows: sqlparser::ast::OffsetRows::None,
        })
    };

    // Use sorting from the frame
    let order_by = sorts
        .last()
        .map(|sorts| {
            sorts
                .iter()
                .map(|s| translate_column_sort(s, ctx))
                .try_collect()
        })
        .transpose()?
        .unwrap_or_default();

    let (top, limit) = if ctx.dialect.use_top() {
        (limit.map(|l| top_of_i64(l, ctx)), None)
    } else {
        (None, limit.map(expr_of_i64))
    };

    ctx.pop_query();

    Ok(sql_ast::Query {
        order_by,
        limit,
        offset,
        ..default_query(SetExpr::Select(Box::new(Select {
            distinct,
            top,
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
) -> Result<sql_ast::Query, anyhow::Error> {
    // reverse, so it's easier (and O(1)) to pop
    pipeline.reverse();

    while let Some(transform) = pipeline.pop() {
        use SqlTransform::*;

        let op = match &transform {
            Union { .. } => sql_ast::SetOperator::Union,
            Except { .. } => sql_ast::SetOperator::Except,
            Intersect { .. } => sql_ast::SetOperator::Intersect,
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
    Ok(match relation_expr {
        RelationExpr::Ref(tid, alias) => {
            let decl = ctx.anchor.table_decls.get_mut(&tid).unwrap();

            // prepare names
            let table_name = match &decl.name {
                None => {
                    decl.name = Some(ctx.anchor.table_name.gen());
                    decl.name.clone().unwrap()
                }
                Some(n) => n.clone(),
            };

            let name = sql_ast::ObjectName(translate_ident(Some(table_name.clone()), None, ctx));

            TableFactor::Table {
                name,
                alias: if Some(table_name) == alias {
                    None
                } else {
                    translate_table_alias(alias, ctx)
                },
                args: None,
                with_hints: vec![],
            }
        }
        RelationExpr::SubQuery(query, alias) => {
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

    let constraint = JoinConstraint::On(translate_expr(filter, ctx)?);

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

#[allow(dead_code, unused_variables)]
fn sql_of_loop(pipeline: Vec<SqlTransform>, ctx: &mut Context) -> Result<Vec<SqlTransform>> {
    /*


    // split the pipeline
    let (mut initial, mut following) = pipeline.break_up(|t| matches!(t, SqlTransform::Loop(_)));
    let loop_ = following.remove(0);
    let step = loop_.into_loop().unwrap();

    // RECURSIVE can only follow WITH directly, which means that if we want to use it for
    // an arbitrary query, we have to defined a *nested* WITH RECURSIVE and not use
    // the top-level list of CTEs.

    // determine columns of the initial table
    let recursive_columns = AnchorContext::determine_select_columns(&initial);

    // do the same thing we do when splitting a pipeline
    // (defining new columns, redirecting cids)
    let recursive_columns = SqlTransform::Super(Transform::Select(recursive_columns));
    initial.push(recursive_columns.clone());
    let (step, _) = anchor_split(&mut ctx.anchor, initial, step);
    let from = step.first().unwrap().as_super().unwrap().as_from().unwrap();

    let table_name = "_loop";
    let initial = ctx.anchor.table_decls.get_mut(&from.source).unwrap();
    initial.name = Some(table_name.to_string());
    let initial_relation = if let RelationStatus::NotYetDefined(rel) = initial.relation {
        rel.preprocess(ctx)?
    } else {
        unreachable!()
    };

    let (initial, _) = initial_relation.into_pipeline().unwrap();

    // compile initial
    let initial = query_to_set_expr(sql_query_of_pipeline(initial, ctx)?, ctx);

    // compile step (without producing CTEs)
    ctx.push_query();
    ctx.query.allow_ctes = false;

    let step = query_to_set_expr(sql_query_of_pipeline(step, ctx)?, ctx);

    ctx.pop_query();

    // build CTE and it's SELECT
    let cte = sql_ast::Cte {
        alias: simple_table_alias(Ident::new(table_name)),
        query: Box::new(default_query(SetExpr::SetOperation {
            op: sql_ast::SetOperator::Union,
            set_quantifier: sql_ast::SetQuantifier::All,
            left: initial,
            right: step,
        })),
        from: None,
    };
    let query = Box::new(sql_ast::Query {
        with: Some(sql_ast::With {
            recursive: true,
            cte_tables: vec![cte],
        }),
        ..default_query(sql_ast::SetExpr::Select(Box::new(sql_ast::Select {
            projection: vec![SelectItem::Wildcard(
                sql_ast::WildcardAdditionalOptions::default(),
            )],
            from: vec![TableWithJoins {
                relation: TableFactor::Table {
                    name: sql_ast::ObjectName(vec![Ident::new(table_name)]),
                    alias: None,
                    args: None,
                    with_hints: Vec::new(),
                },
                joins: vec![],
            }],
            ..default_select()
        })))
    });

    // create a split between the loop SELECT statement and the following pipeline
    let (mut following, _) = anchor_split(&mut ctx.anchor, vec![recursive_columns], following);

    let from = following.first_mut().unwrap();
    let from = from.as_super().unwrap().as_from().unwrap();

    // this will be table decl that references the whole loop expression
    let loop_decl = ctx.anchor.table_decls.get_mut(&from.source).unwrap();

    let loop_name = ctx.anchor.table_name.gen();
    loop_decl.name = Some(loop_name.clone());
    loop_decl.relation = RelationStatus::Defined;

    // push the whole thing into WITH of the main query
    ctx.ctes.push((loop_name, query));

    Ok(following)
     */
    todo!()
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
                        alias: sql_ast::Ident::new(col),
                    })
                })
                .try_collect()?,
            ..default_select()
        }));

        selects.push(body)
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

fn filter_of_conditions(exprs: Vec<Expr>, context: &mut Context) -> Result<Option<sql_ast::Expr>> {
    Ok(if let Some(cond) = all(exprs) {
        Some(translate_expr(cond, context)?)
    } else {
        None
    })
}

fn all(mut exprs: Vec<Expr>) -> Option<Expr> {
    let mut condition = exprs.pop()?;
    while let Some(expr) = exprs.pop() {
        condition = Expr {
            kind: ExprKind::BuiltInFunction {
                name: super::std::STD_AND.name.to_string(),
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
    }
}

fn default_select() -> Select {
    Select {
        distinct: false,
        top: None,
        projection: Vec::new(),
        into: None,
        from: Vec::new(),
        lateral_views: Vec::new(),
        selection: None,
        group_by: Vec::new(),
        cluster_by: Vec::new(),
        distribute_by: Vec::new(),
        sort_by: Vec::new(),
        having: None,
        qualify: None,
    }
}

fn simple_table_alias(name: Ident) -> TableAlias {
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
                alias: Some(simple_table_alias(Ident::new(
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
        group [title, emp_no] (
            aggregate [emp_salary = average salary]
        )
        group [title] (
            aggregate [avg_salary = average emp_salary]
        )
        "#;

        let sql_ast = crate::tests::compile(query).unwrap();

        assert_snapshot!(sql_ast, @r###"
        WITH table_1 AS (
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
          table_1 AS table_0
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
        derive global_rank = rank
        filter country == "USA"
        derive rank = rank
        "#;

        let sql_ast = crate::tests::compile(query).unwrap();

        assert_snapshot!(sql_ast, @r###"
        WITH table_1 AS (
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
          table_1 AS table_0
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
        WITH table_1 AS (
          SELECT
            *,
            AVG(bar) OVER () AS _expr_0
          FROM
            tbl1
        )
        SELECT
          *
        FROM
          table_1 AS table_0
        WHERE
          _expr_0 > 3
        "###);
    }
}
