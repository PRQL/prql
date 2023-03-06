//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, Ident, Join, JoinConstraint, JoinOperator, Select, SelectItem, SetExpr,
    TableAlias, TableFactor, TableWithJoins,
};

use crate::ast::pl::{BinOp, JoinSide, Literal, RelationLiteral};
use crate::ast::rq::{CId, Expr, ExprKind, Query, RelationKind, TableRef, Transform};
use crate::sql::anchor::anchor_split;
use crate::sql::preprocess::SqlRelationKind;
use crate::utils::{BreakUp, Pluck};

use crate::Target;

use super::context::AnchorContext;
use super::gen_expr::*;
use super::gen_projection::*;
use super::preprocess::{self, SqlRelation, SqlTransform};
use super::{anchor, Context, Dialect};

pub fn translate_query(query: Query, dialect: Option<Dialect>) -> Result<sql_ast::Query> {
    let dialect = if let Some(dialect) = dialect {
        dialect
    } else {
        let target = query.def.other.get("target");
        let Target::Sql(maybe_dialect) = target
            .map(|s| Target::from_str(s))
            .transpose()?
            .unwrap_or_default();
        maybe_dialect.unwrap_or_default()
    };
    let dialect = dialect.handler();

    let (anchor, main_relation) = AnchorContext::of(query);

    let mut ctx = Context::new(dialect, anchor);

    // compile main relation that will recursively compile CTEs
    let mut main_query = sql_query_of_sql_relation(main_relation.into(), &mut ctx)?;

    // attach CTEs
    if !ctx.ctes.is_empty() {
        main_query.with = Some(sql_ast::With {
            cte_tables: ctx.ctes.drain(..).collect_vec(),
            recursive: false,
        });
    }

    Ok(main_query)
}

fn sql_query_of_sql_relation(
    sql_relation: SqlRelation,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    use RelationKind::*;

    // preprocess & split into atomics
    match sql_relation.kind {
        // base case
        SqlRelationKind::Super(Pipeline(pipeline)) => {
            // preprocess
            let pipeline = Ok(pipeline)
                .map(preprocess::normalize)
                .map(preprocess::prune_inputs)
                .map(preprocess::wrap)
                .and_then(|p| preprocess::distinct(p, ctx))
                .map(preprocess::union)
                .and_then(|p| preprocess::except(p, ctx))
                .and_then(|p| preprocess::intersect(p, ctx))
                .map(preprocess::reorder)?;

            // load names of output columns
            ctx.anchor.load_names(&pipeline, sql_relation.columns);

            sql_query_of_pipeline(pipeline, ctx)
        }

        // no need to preprocess, has been done already
        SqlRelationKind::PreprocessedPipeline(pipeline) => sql_query_of_pipeline(pipeline, ctx),

        // special case: literals
        SqlRelationKind::Super(Literal(lit)) => sql_of_sample_data(lit, ctx),

        // special case: s-strings
        SqlRelationKind::Super(SString(items)) => translate_query_sstring(items, ctx),

        // ref cannot be converted directly into query and does not need it's own CTE
        SqlRelationKind::Super(ExternRef(_)) => unreachable!(),
    }
}

fn table_factor_of_table_ref(table_ref: TableRef, ctx: &mut Context) -> Result<TableFactor> {
    let table_ref_alias = (table_ref.name.clone())
        .map(|ident| translate_ident_part(ident, ctx))
        .map(simple_table_alias);

    let decl = ctx.anchor.table_decls.get_mut(&table_ref.source).unwrap();

    // prepare names
    let table_name = match &decl.name {
        None => {
            decl.name = Some(ctx.anchor.table_name.gen());
            decl.name.clone().unwrap()
        }
        Some(n) => n.clone(),
    };

    // ensure that the table is declared
    if let Some(sql_relation) = decl.relation.take() {
        // if we cannot use CTEs
        if !ctx.query.allow_ctes {
            // restore relation for other references
            decl.relation = Some(sql_relation.clone());

            // return a sub-query
            let query = sql_query_of_sql_relation(sql_relation, ctx)?;
            return Ok(TableFactor::Derived {
                lateral: false,
                subquery: Box::new(query),
                alias: table_ref_alias,
            });
        }

        let query = sql_query_of_sql_relation(sql_relation, ctx)?;
        let alias = sql_ast::TableAlias {
            name: translate_ident_part(table_name.clone(), ctx),
            columns: vec![],
        };

        ctx.ctes.push(sql_ast::Cte {
            alias,
            query: Box::new(query),
            from: None,
        })
    }

    // let name = match &decl.relation {
    //     // special case for anchor
    //     // TODO
    //     // Some(SqlRelationKind::Super(RelationKind::ExternRef(TableExternRef::Anchor(
    //     // anchor_id,
    //     // )))) => sql_ast::ObjectName(vec![Ident::new(anchor_id.clone())]),

    //     // base case
    //     _ => {

    //     }
    // };

    let name = sql_ast::ObjectName(translate_ident(Some(table_name.clone()), None, ctx));

    Ok(TableFactor::Table {
        name,
        alias: if Some(table_name) == table_ref.name {
            None
        } else {
            table_ref_alias
        },
        args: None,
        with_hints: vec![],
        columns_definition: None,
    })
}

fn translate_join(
    (side, with, filter): (JoinSide, TableRef, Expr),
    ctx: &mut Context,
) -> Result<Join> {
    let relation = table_factor_of_table_ref(with, ctx)?;

    let constraint = JoinConstraint::On(translate_expr_kind(filter.kind, ctx)?);

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

fn sql_query_of_pipeline(
    mut pipeline: Vec<SqlTransform>,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    use SqlTransform::*;

    // special case: loop
    if pipeline.iter().any(|t| matches!(t, Loop(_))) {
        pipeline = sql_of_loop(pipeline, ctx)?;
    }

    // extract an atomic pipeline from back of the pipeline and stash preceding part into context
    let pipeline = extract_atomic(pipeline, &mut ctx.anchor);

    // ensure names for all columns that need it
    ensure_names(&pipeline, &mut ctx.anchor);

    let (select, set_ops) =
        pipeline.break_up(|t| matches!(t, Union { .. } | Except { .. } | Intersect { .. }));

    let select = sql_select_query_of_pipeline(select, ctx)?;

    sql_set_ops_of_pipeline(select, set_ops, ctx)
}

fn sql_select_query_of_pipeline(
    mut pipeline: Vec<SqlTransform>,
    ctx: &mut Context,
) -> Result<sql_ast::Query> {
    let table_count = count_tables(&pipeline);
    log::debug!("atomic query contains {table_count} tables");
    ctx.push_query();
    ctx.query.omit_ident_prefix = table_count == 1;
    ctx.query.pre_projection = true;

    let mut from: Vec<_> = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_from()))
        .into_iter()
        .map(|source| -> Result<TableWithJoins> {
            Ok(TableWithJoins {
                relation: table_factor_of_table_ref(source, ctx)?,
                joins: vec![],
            })
        })
        .try_collect()?;

    let joins = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_join()))
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
        .pluck(|t| t.into_super_and(|t| t.into_select()))
        .into_iter()
        .exactly_one()
        .unwrap();
    let projection = translate_wildcards(&ctx.anchor, projection);
    let projection = translate_select_items(projection.0, projection.1, ctx)?;

    let sorts = pipeline.pluck(|t| t.into_super_and(|t| t.into_sort()));
    let takes = pipeline.pluck(|t| t.into_super_and(|t| t.into_take()));
    let distinct = pipeline.iter().any(|t| matches!(t, SqlTransform::Distinct));

    // Split the pipeline into before & after the aggregate
    let (mut before_agg, mut after_agg) = pipeline.break_up(|t| {
        matches!(
            t,
            SqlTransform::Super(Transform::Aggregate { .. } | Transform::Append(_))
        )
    });

    // WHERE and HAVING
    let where_ = filter_of_conditions(
        before_agg.pluck(|t| t.into_super_and(|t| t.into_filter())),
        ctx,
    )?;
    let having = filter_of_conditions(
        after_agg.pluck(|t| t.into_super_and(|t| t.into_filter())),
        ctx,
    )?;

    // GROUP BY
    let aggregate = after_agg
        .pluck(|t| t.into_super_and(|t| t.into_aggregate()))
        .into_iter()
        .next();
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
        Some(sqlparser::ast::Offset {
            value: translate_expr_kind(ExprKind::Literal(Literal::Integer(offset)), ctx)?,
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

fn sql_set_ops_of_pipeline(
    mut top: sql_ast::Query,
    mut pipeline: Vec<SqlTransform>,
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
            right: Box::new(SetExpr::Select(Box::new(Select {
                projection: vec![SelectItem::Wildcard(
                    sql_ast::WildcardAdditionalOptions::default(),
                )],
                from: vec![TableWithJoins {
                    relation: table_factor_of_table_ref(bottom, context)?,
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

fn sql_of_loop(pipeline: Vec<SqlTransform>, ctx: &mut Context) -> Result<Vec<SqlTransform>> {
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
    let step = anchor_split(&mut ctx.anchor, initial, step);
    let from = step.first().unwrap().as_super().unwrap().as_from().unwrap();

    let initial = ctx.anchor.table_decls.get_mut(&from.source).unwrap();
    initial.name = Some("loop".to_string());
    let initial_relation = initial.relation.take().unwrap();

    let initial = initial_relation.kind.into_preprocessed_pipeline().unwrap();

    // compile initial
    let initial = query_to_set_expr(sql_query_of_pipeline(initial, ctx)?, ctx);

    // compile step (without producing CTEs)
    ctx.push_query();
    ctx.query.allow_ctes = false;

    let step = query_to_set_expr(sql_query_of_pipeline(step, ctx)?, ctx);

    ctx.pop_query();

    // build CTE and it's SELECT
    let cte = sql_ast::Cte {
        alias: simple_table_alias(Ident::new("loop")),
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
                    name: sql_ast::ObjectName(vec![Ident::new("loop")]),
                    alias: None,
                    args: None,
                    with_hints: Vec::new(),
                    columns_definition: None,
                },
                joins: vec![],
            }],
            ..default_select()
        })))
    });

    // create a split between the loop SELECT statement and the following pipeline
    let mut following = anchor_split(&mut ctx.anchor, vec![recursive_columns], following);

    let from = following.first_mut().unwrap();
    let from = from.as_super().unwrap().as_from().unwrap();

    // this will be table decl that references the whole loop expression
    let loop_decl = ctx.anchor.table_decls.get_mut(&from.source).unwrap();

    let loop_name = ctx.anchor.table_name.gen();
    loop_decl.name = Some(loop_name.clone());
    loop_decl.relation = None;

    // push the whole thing into WITH of the main query
    ctx.ctes.push(sql_ast::Cte {
        alias: simple_table_alias(Ident::new(loop_name)),
        query,
        from: None,
    });

    Ok(following)
}

fn sql_of_sample_data(data: RelationLiteral, ctx: &Context) -> Result<sql_ast::Query> {
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

/// Extract last part of pipeline that is able to "fit" into a single SELECT statement.
/// Remaining proceeding pipeline is declared as a table and stored in AnchorContext.
fn extract_atomic(pipeline: Vec<SqlTransform>, ctx: &mut AnchorContext) -> Vec<SqlTransform> {
    let (preceding, atomic) = anchor::split_off_back(pipeline, ctx);

    if let Some(preceding) = preceding {
        log::debug!(
            "pipeline split after {}",
            preceding.last().unwrap().as_str()
        );

        anchor::anchor_split(ctx, preceding, atomic)
    } else {
        atomic
    }

    // TODO
    // sometimes, additional columns will be added into select, which have to
    // be filtered out here, using additional CTE
    // if let Some((pipeline, _)) = parts.last() {
    //     let select_cols = pipeline
    //         .first()
    //         .unwrap()
    //         .as_super()
    //         .unwrap()
    //         .as_select()
    //         .unwrap();

    //     if select_cols.iter().any(|c| !outputs_cid.contains(c)) {
    //         parts.push((
    //             vec![SqlTransform::Super(Transform::Select(outputs_cid))],
    //             select_cols.clone(),
    //         ));
    //     }
    // }
}

fn ensure_names(transforms: &[SqlTransform], ctx: &mut AnchorContext) {
    let empty = HashSet::new();
    for t in transforms {
        match t {
            SqlTransform::Super(Transform::Sort(_)) => {
                for r in anchor::get_requirements(t, &empty) {
                    ctx.ensure_column_name(r.col);
                }
            }
            SqlTransform::Super(Transform::Select(cids)) => {
                for cid in cids {
                    let _decl = &ctx.column_decls[cid];
                    //let name = match decl {
                    //    ColumnDecl::RelationColumn(_, _, _) => todo!(),
                    //    ColumnDecl::Compute(_) => ctx.column_names[..],
                    //};
                }
            }
            _ => (),
        }
    }
}
fn filter_of_conditions(exprs: Vec<Expr>, context: &mut Context) -> Result<Option<sql_ast::Expr>> {
    Ok(if let Some(cond) = all(exprs) {
        Some(translate_expr_kind(cond.kind, context)?)
    } else {
        None
    })
}

fn all(mut exprs: Vec<Expr>) -> Option<Expr> {
    let mut condition = exprs.pop()?;
    while let Some(expr) = exprs.pop() {
        condition = Expr {
            kind: ExprKind::Binary {
                op: BinOp::And,
                left: Box::new(expr),
                right: Box::new(condition),
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

fn count_tables(transforms: &[SqlTransform]) -> usize {
    let mut count = 0;
    for transform in transforms {
        if let SqlTransform::Super(Transform::Join { .. } | Transform::From(_)) = transform {
            count += 1;
        }
    }

    count
}
#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::*;
    use crate::{parser::parse, semantic::resolve, sql::dialect::GenericDialect};

    fn parse_and_resolve(prql: &str) -> Result<(Vec<SqlTransform>, Context)> {
        let query = resolve(parse(prql)?)?;
        let (anchor, main_relation) = AnchorContext::of(query);
        let context = Context::new(Box::new(GenericDialect {}), anchor);

        let pipeline = main_relation.kind.into_pipeline().unwrap();

        Ok((preprocess::reorder(preprocess::wrap(pipeline)), context))
    }

    fn count_atomics(prql: &str) -> usize {
        let (mut pipeline, mut context) = parse_and_resolve(prql).unwrap();
        context.anchor.table_decls.clear();

        let mut atomics = 0;
        loop {
            let _ = extract_atomic(pipeline, &mut context.anchor);
            atomics += 1;

            if let Some((_, decl)) = context.anchor.table_decls.drain().next() {
                if let Some(relation) = decl.relation {
                    if let SqlRelationKind::PreprocessedPipeline(p) = relation.kind {
                        pipeline = p;
                        continue;
                    }
                }
            }
            break;
        }
        atomics
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

        assert_eq!(count_atomics(prql), 1);

        // One aggregate, but take at the top
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        "###;

        assert_eq!(count_atomics(prql), 2);

        // A take, then two aggregates
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        aggregate [sal2 = average sal]
        sort sal2
        "###;

        assert_eq!(count_atomics(prql), 3);

        // A take, then a select
        let prql: &str = r###"
        from employees
        take 20
        select first_name
        "###;

        assert_eq!(count_atomics(prql), 1);
    }

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

        let sql_ast = crate::test::compile(query).unwrap();

        assert_snapshot!(sql_ast);
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

        let sql_ast = crate::test::compile(query).unwrap();

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

        assert_snapshot!(crate::test::compile(query).unwrap(), @r###"
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
