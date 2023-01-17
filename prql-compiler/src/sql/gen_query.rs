//! This module is responsible for translating PRQL AST to sqlparser AST, and
//! then to a String. We use sqlparser because it's trivial to create the string
//! once it's in their AST (it's just `.to_string()`). It also lets us support a
//! few dialects of SQL immediately.
use std::collections::HashSet;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, Ident, Select, SelectItem, SetExpr, TableAlias, TableFactor, TableWithJoins,
};

use crate::ast::pl::{BinOp, Literal, RelationLiteral};
use crate::ast::rq::{CId, Expr, ExprKind, Query, Relation, RelationKind, TableDecl, Transform};
use crate::error::{Error, Reason};
use crate::utils::{BreakUp, IntoOnly, Pluck};

use super::context::AnchorContext;
use super::gen_expr::*;
use super::gen_projection::*;
use super::preprocess::{self, SqlTransform};
use super::{anchor, Context, Dialect};

pub fn translate_query(query: Query, dialect: Option<Dialect>) -> Result<sql_ast::Query> {
    let dialect = if let Some(dialect) = dialect {
        dialect
    } else {
        let target = query.def.other.get("target");
        target
            .and_then(|target| target.strip_prefix("sql."))
            .map(|dialect| {
                super::Dialect::from_str(dialect).map_err(|_| {
                    Error::new(Reason::NotFound {
                        name: format!("{dialect:?}"),
                        namespace: "dialect".to_string(),
                    })
                })
            })
            .transpose()?
            .unwrap_or_default()
    };
    let dialect = dialect.handler();

    let (anchor, query) = AnchorContext::of(query);

    let mut context = Context {
        dialect,
        anchor,
        omit_ident_prefix: false,
        pre_projection: false,
    };

    // extract tables and the pipeline
    let tables = into_tables(query.relation, query.tables, &mut context)?;

    // preprocess & split into atomics
    let mut atomics = Vec::new();
    for table in tables {
        let name = table
            .name
            .unwrap_or_else(|| context.anchor.table_name.gen());

        match table.relation.kind {
            RelationKind::Pipeline(pipeline) => {
                // preprocess
                let pipeline = Ok(pipeline)
                    .map(preprocess::wrap)
                    .and_then(|p| preprocess::distinct(p, &mut context))
                    .map(preprocess::reorder)
                    .unwrap();

                // load names of output columns
                context.anchor.load_names(&pipeline, table.relation.columns);

                // split to atomics
                let ats = split_into_atomics(name, pipeline, &mut context.anchor);

                // ensure names for all columns that need it
                ensure_names(&ats, &mut context.anchor);

                atomics.extend(ats);
            }
            RelationKind::Literal(_) | RelationKind::SString(_) => atomics.push(AtomicQuery {
                name,
                relation: SqlRelation::Super(table.relation.kind),
            }),
            RelationKind::ExternRef(_) => {
                // ref does not need it's own CTE
            }
        }
    }

    // take last table
    let main_query = atomics.remove(atomics.len() - 1);
    let ctes = atomics;

    // convert each of the CTEs
    let ctes: Vec<_> = ctes
        .into_iter()
        .map(|t| table_to_sql_cte(t, &mut context))
        .try_collect()?;

    // convert main query
    let mut main_query = sql_query_of_relation(main_query.relation, &mut context)?;

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
    name: String,
    relation: SqlRelation,
}

#[derive(Debug, EnumAsInner)]
enum SqlRelation {
    Super(RelationKind),
    Pipeline(Vec<SqlTransform>),
}

fn into_tables(
    main_pipeline: Relation,
    tables: Vec<TableDecl>,
    context: &mut Context,
) -> Result<Vec<TableDecl>> {
    let main = TableDecl {
        id: context.anchor.tid.gen(),
        name: None,
        relation: main_pipeline,
    };
    Ok([tables, vec![main]].concat())
}

fn table_to_sql_cte(table: AtomicQuery, context: &mut Context) -> Result<sql_ast::Cte> {
    let alias = sql_ast::TableAlias {
        name: translate_ident_part(table.name, context),
        columns: vec![],
    };
    Ok(sql_ast::Cte {
        alias,
        query: Box::new(sql_query_of_relation(table.relation, context)?),
        from: None,
    })
}

fn sql_query_of_relation(relation: SqlRelation, context: &mut Context) -> Result<sql_ast::Query> {
    use RelationKind::*;

    match relation {
        SqlRelation::Super(ExternRef(_)) | SqlRelation::Super(Pipeline(_)) => unreachable!(),
        SqlRelation::Pipeline(pipeline) => sql_query_of_pipeline(pipeline, context),
        SqlRelation::Super(Literal(lit)) => Ok(sql_of_sample_data(lit)),
        SqlRelation::Super(SString(items)) => translate_query_sstring(items, context),
    }
}

fn sql_query_of_pipeline(
    pipeline: Vec<SqlTransform>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    let table_count = count_tables(&pipeline);
    log::debug!("atomic query contains {table_count} tables");
    context.omit_ident_prefix = table_count == 1;

    let (before_append, after_append) =
        pipeline.break_up(|t| matches!(t, SqlTransform::Super(Transform::Append(_))));

    let select = sql_select_query_of_pipeline(before_append, context)?;

    sql_union_of_pipeline(select, after_append, context)
}

fn sql_select_query_of_pipeline(
    mut pipeline: Vec<SqlTransform>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    context.pre_projection = true;

    let projection = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_select()))
        .into_only()
        .unwrap();
    let projection = translate_wildcards(&context.anchor, projection);
    let projection = translate_select_items(projection.0, projection.1, context)?;

    let mut from = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_from()))
        .into_iter()
        .map(|source| TableWithJoins {
            relation: table_factor_of_tid(source, context),
            joins: vec![],
        })
        .collect::<Vec<_>>();

    let joins = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_join()))
        .into_iter()
        .map(|j| translate_join(j, context))
        .collect::<Result<Vec<_>>>()?;
    if !joins.is_empty() {
        if let Some(from) = from.last_mut() {
            from.joins = joins;
        } else {
            return Err(anyhow!("Cannot use `join` without `from`"));
        }
    }

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
        context,
    )?;
    let having = filter_of_conditions(
        after_agg.pluck(|t| t.into_super_and(|t| t.into_filter())),
        context,
    )?;

    // GROUP BY
    let aggregate = after_agg
        .pluck(|t| t.into_super_and(|t| t.into_aggregate()))
        .into_iter()
        .next();
    let group_by: Vec<CId> = aggregate.map(|(part, _)| part).unwrap_or_default();
    let group_by = try_into_exprs(group_by, context, None)?;

    context.pre_projection = false;

    let ranges = takes.into_iter().map(|x| x.range).collect();
    let take = range_of_ranges(ranges)?;
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
    let order_by = sorts
        .last()
        .map(|sorts| {
            sorts
                .iter()
                .map(|s| translate_column_sort(s, context))
                .try_collect()
        })
        .transpose()?
        .unwrap_or_default();

    Ok(sql_ast::Query {
        body: Box::new(SetExpr::Select(Box::new(Select {
            distinct,
            top: if context.dialect.use_top() {
                limit.map(|l| top_of_i64(l, context))
            } else {
                None
            },
            projection,
            into: None,
            from,
            lateral_views: vec![],
            selection: where_,
            group_by,
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
        locks: vec![],
    })
}

fn sql_union_of_pipeline(
    top: sql_ast::Query,
    mut pipeline: Vec<SqlTransform>,
    context: &mut Context,
) -> Result<sql_ast::Query, anyhow::Error> {
    // union

    let append = pipeline
        .pluck(|t| t.into_super_and(|t| t.into_append()))
        .into_iter()
        .next();
    let distinct = pipeline.iter().any(|t| matches!(t, SqlTransform::Distinct));

    let Some(bottom) = append else {
        return Ok(top);
    };

    let top_is_simple = top.with.is_none()
        && top.order_by.is_empty()
        && top.limit.is_none()
        && top.offset.is_none()
        && top.fetch.is_none()
        && top.locks.is_empty();

    let left = if top_is_simple {
        top.body
    } else {
        // top is not simple, so we need to wrap it into
        // `SELECT * FROM top`
        Box::new(SetExpr::Select(Box::new(Select {
            projection: vec![SelectItem::Wildcard(
                sql_ast::WildcardAdditionalOptions::default(),
            )],
            from: vec![TableWithJoins {
                relation: TableFactor::Derived {
                    lateral: false,
                    subquery: Box::new(top),
                    alias: Some(TableAlias {
                        name: Ident::new(context.anchor.table_name.gen()),
                        columns: Vec::new(),
                    }),
                },
                joins: vec![],
            }],
            ..default_select()
        })))
    };
    let op = sql_ast::SetOperator::Union;

    Ok(default_query(SetExpr::SetOperation {
        left,
        right: Box::new(SetExpr::Select(Box::new(Select {
            projection: vec![SelectItem::Wildcard(
                sql_ast::WildcardAdditionalOptions::default(),
            )],
            from: vec![TableWithJoins {
                relation: table_factor_of_tid(bottom, context),
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
    }))
}

fn sql_of_sample_data(data: RelationLiteral) -> sql_ast::Query {
    // TODO: this could be made to use VALUES instead of SELECT UNION ALL SELECT
    //       I'm not sure about compatibility though.

    let mut selects = vec![];

    for row in data.rows {
        // This seems *very* verbose. Maybe we put an issue into sqlparser-rs to
        // have something like a builder for these?
        let body = sql_ast::SetExpr::Select(Box::new(Select {
            distinct: false,
            top: None,
            from: vec![],
            projection: std::iter::zip(data.columns.clone(), row)
                .map(|(col, value)| SelectItem::ExprWithAlias {
                    expr: sql_ast::Expr::Identifier(sql_ast::Ident::new(value)),
                    alias: sql_ast::Ident::new(col),
                })
                .collect(),
            selection: None,
            group_by: vec![],
            having: None,
            lateral_views: vec![],
            cluster_by: vec![],
            distribute_by: vec![],
            into: None,
            qualify: None,
            sort_by: vec![],
        }));

        selects.push(body)
    }

    // Not the most elegant way of doing this but sufficient for now.
    let first = selects.remove(0);
    let body = selects
        .into_iter()
        .fold(first, |acc, select| SetExpr::SetOperation {
            op: sql_ast::SetOperator::Union,
            set_quantifier: sql_ast::SetQuantifier::All,
            left: Box::new(acc),
            right: Box::new(select),
        });

    sql_ast::Query {
        with: (None),
        body: Box::new(body),
        order_by: vec![],
        limit: None,
        offset: None,
        fetch: None,
        locks: vec![],
    }
}

fn split_into_atomics(
    name: String,
    mut pipeline: Vec<SqlTransform>,
    ctx: &mut AnchorContext,
) -> Vec<AtomicQuery> {
    let outputs_cid = AnchorContext::determine_select_columns(&pipeline);

    let mut required_cols = outputs_cid.clone();

    // split pipeline, back to front
    let mut parts_rev = Vec::new();
    loop {
        let (preceding, split) = anchor::split_off_back(ctx, required_cols, pipeline);

        if let Some((preceding, cols_at_split)) = preceding {
            log::debug!(
                "pipeline split after {}",
                preceding.last().unwrap().as_str()
            );
            parts_rev.push((split, cols_at_split.clone()));

            pipeline = preceding;
            required_cols = cols_at_split;
        } else {
            parts_rev.push((split, Vec::new()));
            break;
        }
    }
    parts_rev.reverse();
    let mut parts = parts_rev;

    // sometimes, additional columns will be added into select, which have to
    // be filtered out here, using additional CTE
    if let Some((pipeline, _)) = parts.last() {
        let select_cols = pipeline
            .first()
            .unwrap()
            .as_super()
            .unwrap()
            .as_select()
            .unwrap();

        if select_cols.iter().any(|c| !outputs_cid.contains(c)) {
            parts.push((
                vec![SqlTransform::Super(Transform::Select(outputs_cid))],
                select_cols.clone(),
            ));
        }
    }

    // add names to pipelines, anchor, front to back
    let mut atomics = Vec::with_capacity(parts.len());
    let last = parts.pop().unwrap();

    let last_pipeline = if parts.is_empty() {
        last.0
    } else {
        // this code chunk is bloated but I cannot find a more concise alternative
        let first = parts.remove(0);

        let first_name = ctx.table_name.gen();
        atomics.push(AtomicQuery {
            name: first_name.clone(),
            relation: SqlRelation::Pipeline(first.0),
        });

        let mut prev_name = first_name;
        for (pipeline, cols_before) in parts.into_iter() {
            let name = ctx.table_name.gen();
            let pipeline = anchor::anchor_split(ctx, &prev_name, &cols_before, pipeline);

            atomics.push(AtomicQuery {
                name: name.clone(),
                relation: SqlRelation::Pipeline(pipeline),
            });

            prev_name = name;
        }

        anchor::anchor_split(ctx, &prev_name, &last.1, last.0)
    };
    atomics.push(AtomicQuery {
        name,
        relation: SqlRelation::Pipeline(last_pipeline),
    });

    atomics
}

fn ensure_names(atomics: &[AtomicQuery], ctx: &mut AnchorContext) {
    // ensure column names for columns that need it
    for a in atomics {
        let empty = HashSet::new();
        for t in a.relation.as_pipeline().unwrap() {
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
        let (anchor, query) = AnchorContext::of(query);
        let context = Context {
            dialect: Box::new(GenericDialect {}),
            anchor,
            omit_ident_prefix: false,
            pre_projection: false,
        };

        let pipeline = query.relation.kind.into_pipeline().unwrap();

        Ok((preprocess::reorder(preprocess::wrap(pipeline)), context))
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

        let (pipeline, mut context) = parse_and_resolve(prql).unwrap();
        let queries = split_into_atomics("".to_string(), pipeline, &mut context.anchor);
        assert_eq!(queries.len(), 1);

        // One aggregate, but take at the top
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        sort sal
        "###;

        let (pipeline, mut context) = parse_and_resolve(prql).unwrap();
        let queries = split_into_atomics("".to_string(), pipeline, &mut context.anchor);
        assert_eq!(queries.len(), 2);

        // A take, then two aggregates
        let prql: &str = r###"
        from employees
        take 20
        filter country == "USA"
        aggregate [sal = average salary]
        aggregate [sal2 = average sal]
        sort sal2
        "###;

        let (pipeline, mut context) = parse_and_resolve(prql).unwrap();
        let queries = split_into_atomics("".to_string(), pipeline, &mut context.anchor);
        assert_eq!(queries.len(), 3);

        // A take, then a select
        let prql: &str = r###"
        from employees
        take 20
        select first_name
        "###;

        let (pipeline, mut context) = parse_and_resolve(prql).unwrap();
        let queries = split_into_atomics("".to_string(), pipeline, &mut context.anchor);
        assert_eq!(queries.len(), 1);
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
          table_1
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
          table_1
        WHERE
          _expr_0 > 3
        "###);
    }
}
