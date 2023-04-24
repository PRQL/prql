//! Contains functions that compile [crate::ast::pl] nodes into [sqlparser] nodes.

use anyhow::{bail, Result};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use sqlparser::ast::{
    self as sql_ast, BinaryOperator, DateTimeField, Function, FunctionArg, FunctionArgExpr,
    ObjectName, OrderByExpr, SelectItem, Top, UnaryOperator, Value, WindowFrameBound, WindowSpec,
};
use sqlparser::keywords::{
    Keyword, ALL_KEYWORDS, ALL_KEYWORDS_INDEX, RESERVED_FOR_COLUMN_ALIAS, RESERVED_FOR_TABLE_ALIAS,
};
use std::collections::HashSet;

use crate::ast::pl::{
    ColumnSort, InterpolateItem, Literal, Range, SortDirection, WindowFrame, WindowKind,
};
use crate::ast::rq::*;
use crate::error::{Error, Span};
use crate::sql::context::ColumnDecl;
use crate::utils::OrMap;

use super::gen_projection::try_into_exprs;
use super::std::*;
use super::Context;

#[allow(deprecated)]
pub(super) fn translate_expr(expr: Expr, ctx: &mut Context) -> Result<sql_ast::Expr> {
    Ok(match expr.kind {
        ExprKind::ColumnRef(cid) => translate_cid(cid, ctx)?,

        // Fairly hacky — convert everything to a string, then concat it,
        // then convert to sql_ast::Expr. We can't use the `Item::sql_ast::Expr` code above
        // since we don't want to intersperse with spaces.
        ExprKind::SString(s_string_items) => {
            let string = translate_sstring(s_string_items, ctx)?;

            sql_ast::Expr::Identifier(sql_ast::Ident::new(string))
        }
        ExprKind::Param(id) => sql_ast::Expr::Identifier(sql_ast::Ident::new(format!("${id}"))),
        ExprKind::Literal(l) => translate_literal(l, ctx)?,
        ExprKind::Case(mut cases) => {
            let default = cases
                .last()
                .filter(|last| {
                    matches!(
                        last.condition.kind,
                        ExprKind::Literal(Literal::Boolean(true))
                    )
                })
                .map(|def| translate_expr(def.value.clone(), ctx))
                .transpose()?;

            if default.is_some() {
                cases.pop();
            }

            let else_result = default
                .or(Some(sql_ast::Expr::Value(Value::Null)))
                .map(Box::new);

            let cases: Vec<_> = cases
                .into_iter()
                .map(|case| -> Result<_> {
                    let cond = translate_expr(case.condition, ctx)?;
                    let value = translate_expr(case.value, ctx)?;
                    Ok((cond, value))
                })
                .try_collect()?;
            let (conditions, results) = cases.into_iter().unzip();

            sql_ast::Expr::Case {
                operand: None,
                conditions,
                results,
                else_result,
            }
        }
        ExprKind::BuiltInFunction { .. } => {
            let expr = match try_into_is_null(expr, ctx)? {
                Ok(is_null) => return Ok(is_null),
                Err(expr) => expr,
            };

            let expr = match try_into_between(expr, ctx)? {
                Ok(between) => return Ok(between),
                Err(expr) => expr,
            };

            let expr = match try_into_concat_function(expr, ctx)? {
                Ok(between) => return Ok(between),
                Err(expr) => expr,
            };

            let expr = match try_into_binary_op(expr, ctx)? {
                Ok(bin_op) => return Ok(bin_op),
                Err(expr) => expr,
            };

            let expr = match try_into_unary_op(expr, ctx)? {
                Ok(un_op) => return Ok(un_op),
                Err(expr) => expr,
            };

            super::std::translate_built_in(expr, ctx)?
        }
    })
}

fn try_into_binary_op(expr: Expr, ctx: &mut Context) -> Result<Result<sql_ast::Expr, Expr>> {
    use BinaryOperator::*;
    const DECLS: [super::std::FunctionDecl<2>; 14] = [
        STD_MUL, STD_DIV, STD_MOD, STD_ADD, STD_SUB, STD_EQ, STD_NE, STD_GT, STD_LT, STD_GTE,
        STD_LTE, STD_AND, STD_OR, STD_CONCAT,
    ];
    const OPS: [BinaryOperator; 14] = [
        Multiply,
        Divide,
        Modulo,
        Plus,
        Minus,
        Eq,
        NotEq,
        Gt,
        Lt,
        GtEq,
        LtEq,
        And,
        Or,
        StringConcat,
    ];

    let Some((decl, _)) = try_unpack(&expr, DECLS)? else {
        return Ok(Err(expr));
    };

    // this lookup is O(N), but 13 is not that big of a N
    let decl_index = DECLS.iter().position(|x| x == &decl).unwrap();
    let op = OPS[decl_index].clone();
    let [left, right] = unpack(expr, decl);

    let strength = op.binding_strength();
    let left = translate_operand(left, strength, !op.associates_left(), ctx)?;
    let right = translate_operand(right, strength, !op.associates_right(), ctx)?;
    Ok(Ok(sql_ast::Expr::BinaryOp { left, right, op }))
}

fn try_into_unary_op(expr: Expr, ctx: &mut Context) -> Result<Result<sql_ast::Expr, Expr>> {
    use UnaryOperator::*;
    const DECLS: [super::std::FunctionDecl<1>; 2] = [STD_NEG, STD_NOT];
    const OPS: [UnaryOperator; 2] = [Minus, Not];

    let Some((decl, _)) = try_unpack(&expr, DECLS)? else {
        return Ok(Err(expr));
    };
    // this lookup is O(N), but 13 is not that big of a N
    let decl_index = DECLS.iter().position(|x| x == &decl).unwrap();
    let op = OPS[decl_index];
    let [arg] = unpack(expr, decl);

    let expr = translate_operand(arg, op.binding_strength(), false, ctx)?;
    Ok(Ok(sql_ast::Expr::UnaryOp { op, expr }))
}

fn try_into_concat_function(expr: Expr, ctx: &mut Context) -> Result<Result<sql_ast::Expr, Expr>> {
    if !ctx.dialect.has_concat_function() {
        return Ok(Err(expr));
    }

    let args = match try_unpack_concat(expr)? {
        Ok(args) => args,
        Err(expr) => return Ok(Err(expr)),
    };

    let args = args
        .into_iter()
        .map(|a| {
            translate_expr(a, ctx)
                .map(FunctionArgExpr::Expr)
                .map(FunctionArg::Unnamed)
        })
        .try_collect()?;

    Ok(Ok(sql_ast::Expr::Function(Function {
        name: ObjectName(vec![sql_ast::Ident::new("CONCAT")]),
        args,
        over: None,
        distinct: false,
        special: false,
    })))
}

fn try_unpack_concat(expr: Expr) -> Result<Result<Vec<Expr>, Expr>> {
    let Some((_, _)) = try_unpack(&expr, [STD_CONCAT])? else {
        return Ok(Err(expr));
    };
    let [left, right] = unpack(expr, STD_CONCAT);

    let mut args = match try_unpack_concat(left)? {
        Ok(args) => args,
        Err(left) => vec![left],
    };
    args.push(right);
    Ok(Ok(args))
}

pub(super) fn translate_literal(l: Literal, ctx: &Context) -> Result<sql_ast::Expr> {
    Ok(match l {
        Literal::Null => sql_ast::Expr::Value(Value::Null),
        Literal::String(s) => sql_ast::Expr::Value(Value::SingleQuotedString(s)),
        Literal::Boolean(b) => sql_ast::Expr::Value(Value::Boolean(b)),
        Literal::Float(f) => sql_ast::Expr::Value(Value::Number(format!("{f:?}"), false)),
        Literal::Integer(i) => sql_ast::Expr::Value(Value::Number(format!("{i}"), false)),
        Literal::Date(value) => translate_datetime_literal(sql_ast::DataType::Date, value, ctx),
        Literal::Time(value) => translate_datetime_literal(
            sql_ast::DataType::Time(None, sql_ast::TimezoneInfo::None),
            value,
            ctx,
        ),
        Literal::Timestamp(value) => translate_datetime_literal(
            sql_ast::DataType::Timestamp(None, sql_ast::TimezoneInfo::None),
            value,
            ctx,
        ),
        Literal::ValueAndUnit(vau) => {
            let sql_parser_datetime = match vau.unit.as_str() {
                "years" => DateTimeField::Year,
                "months" => DateTimeField::Month,
                "days" => DateTimeField::Day,
                "hours" => DateTimeField::Hour,
                "minutes" => DateTimeField::Minute,
                "seconds" => DateTimeField::Second,
                _ => bail!("Unsupported interval unit: {}", vau.unit),
            };
            let value = if ctx.dialect.requires_quotes_intervals() {
                Box::new(sql_ast::Expr::Value(Value::SingleQuotedString(
                    vau.n.to_string(),
                )))
            } else {
                Box::new(translate_literal(Literal::Integer(vau.n), ctx)?)
            };
            sql_ast::Expr::Interval {
                value,
                leading_field: Some(sql_parser_datetime),
                leading_precision: None,
                last_field: None,
                fractional_seconds_precision: None,
            }
        }
        Literal::Relation(_) => unreachable!(),
    })
}

fn translate_datetime_literal(
    data_type: sql_ast::DataType,
    value: String,
    ctx: &Context,
) -> sql_ast::Expr {
    if ctx.dialect.is::<crate::sql::dialect::SQLiteDialect>() {
        translate_datetime_literal_with_sqlite_function(data_type, value)
    } else {
        translate_datetime_literal_with_typed_string(data_type, value)
    }
}

fn translate_datetime_literal_with_typed_string(
    data_type: sql_ast::DataType,
    value: String,
) -> sql_ast::Expr {
    sql_ast::Expr::TypedString { data_type, value }
}

fn translate_datetime_literal_with_sqlite_function(
    data_type: sql_ast::DataType,
    value: String,
) -> sql_ast::Expr {
    // TODO: promote parsing timezone handling to the parser; we should be storing
    // structured data rather than strings in the AST
    let timezone_indicator_regex = Regex::new(r"([+-]\d{2}):?(\d{2})$").unwrap();
    let time_value = if let Some(groups) = timezone_indicator_regex.captures(value.as_str()) {
        // formalize the timezone indicator to be [+-]HH:MM
        // ref: https://www.sqlite.org/lang_datefunc.html
        timezone_indicator_regex
            .replace(&value, format!("{}:{}", &groups[1], &groups[2]).as_str())
            .to_string()
    } else {
        value
    };

    let arg = FunctionArg::Unnamed(FunctionArgExpr::Expr(sql_ast::Expr::Value(
        Value::SingleQuotedString(time_value),
    )));

    let func_name = match data_type {
        sql_ast::DataType::Date => data_type.to_string(),
        sql_ast::DataType::Time(..) => data_type.to_string(),
        sql_ast::DataType::Timestamp(..) => "DATETIME".to_string(),
        _ => unreachable!(),
    };

    sql_ast::Expr::Function(Function {
        name: ObjectName(vec![sql_ast::Ident::new(func_name)]),
        args: vec![arg],
        over: None,
        distinct: false,
        special: false,
    })
}

pub(super) fn translate_cid(cid: CId, ctx: &mut Context) -> Result<sql_ast::Expr> {
    if ctx.query.pre_projection {
        log::debug!("translating {cid:?} pre projection");
        let decl = ctx.anchor.column_decls.get(&cid).expect("bad RQ ids");

        Ok(match decl {
            ColumnDecl::Compute(compute) => {
                let window = compute.window.clone();
                let span = compute.expr.span;

                let expr = translate_expr(compute.expr.clone(), ctx)?;

                if let Some(window) = window {
                    translate_windowed(expr, window, ctx, span)?
                } else {
                    expr
                }
            }
            ColumnDecl::RelationColumn(tiid, _, col) => {
                let column = match col.clone() {
                    RelationColumn::Wildcard => translate_star(ctx, None)?,
                    RelationColumn::Single(name) => name.unwrap(),
                };
                let t = &ctx.anchor.table_instances[tiid];

                let ident = translate_ident(t.name.clone(), Some(column), ctx);
                sql_ast::Expr::CompoundIdentifier(ident)
            }
        })
    } else {
        // translate into ident
        let column_decl = &&ctx.anchor.column_decls[&cid];

        let table_name = if let ColumnDecl::RelationColumn(tiid, _, _) = column_decl {
            let t = &ctx.anchor.table_instances[tiid];
            Some(t.name.clone().unwrap())
        } else {
            None
        };

        let column = match &column_decl {
            ColumnDecl::RelationColumn(_, _, RelationColumn::Wildcard) => {
                translate_star(ctx, None)?
            }

            _ => {
                let name = ctx.anchor.column_names.get(&cid).cloned();
                name.expect("name of this column has not been to be set before generating SQL")
            }
        };

        let ident = translate_ident(table_name, Some(column), ctx);

        log::debug!("translating {cid:?} post projection: {ident:?}");

        let ident = sql_ast::Expr::CompoundIdentifier(ident);
        Ok(ident)
    }
}

pub(super) fn translate_star(ctx: &Context, span: Option<Span>) -> Result<String> {
    if !ctx.query.allow_stars {
        Err(
            Error::new_simple("Target dialect does not support * in this position.")
                .with_span(span)
                .into(),
        )
    } else {
        Ok("*".to_string())
    }
}

pub(super) fn translate_sstring(
    items: Vec<InterpolateItem<Expr>>,
    ctx: &mut Context,
) -> Result<String> {
    Ok(items
        .into_iter()
        .map(|s_string_item| match s_string_item {
            InterpolateItem::String(string) => Ok(string),
            InterpolateItem::Expr(node) => translate_expr(*node, ctx).map(|expr| expr.to_string()),
        })
        .collect::<Result<Vec<String>>>()?
        .join(""))
}

pub(super) fn translate_query_sstring(
    items: Vec<crate::ast::pl::InterpolateItem<Expr>>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    let string = translate_sstring(items, context)?;

    let re = Regex::new(r"(?i)^SELECT(\s|\n|\r)").unwrap();
    let prefix = if let Some(string) = string.trim().get(0..7) {
        string
    } else {
        ""
    };

    if re.is_match(prefix) {
        if let Some(string) = string.trim().strip_prefix(prefix) {
            return Ok(sql_ast::Query {
                body: Box::new(sql_ast::SetExpr::Select(Box::new(sql_ast::Select {
                    projection: vec![sql_ast::SelectItem::UnnamedExpr(sql_ast::Expr::Identifier(
                        sql_ast::Ident::new(string),
                    ))],
                    distinct: false,
                    top: None,
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
                }))),
                with: None,
                order_by: Vec::new(),
                limit: None,
                offset: None,
                fetch: None,
                locks: vec![],
            });
        }
    }

    bail!(
        Error::new_simple("s-strings representing a table must start with `SELECT `".to_string())
            .with_help("this is a limitation by current compiler implementation")
    )
}

/// Aggregate several ordered ranges into one, computing the intersection.
///
/// Returns a tuple of `(start, end)`, where `end` is optional.
pub(super) fn range_of_ranges(ranges: Vec<Range<Expr>>) -> Result<Range<i64>> {
    let mut current = Range::default();
    for range in ranges {
        let mut range = try_range_into_int(range)?;

        // b = b + a.start -1 (take care of 1-based index!)
        range.start = range.start.or_map(current.start, |a, b| a + b - 1);
        range.end = range.end.map(|b| current.start.unwrap_or(1) + b - 1);

        // b.end = min(a.end, b.end)
        range.end = current.end.or_map(range.end, i64::min);
        current = range;
    }

    if let Some((s, e)) = current.start.zip(current.end) {
        if e < s {
            return Ok(Range {
                start: None,
                end: Some(0),
            });
        }
    }
    Ok(current)
}

fn try_range_into_int(range: Range<Expr>) -> Result<Range<i64>> {
    fn cast_bound(bound: Expr) -> Result<i64> {
        Ok(bound.kind.into_literal()?.into_integer()?)
    }

    Ok(Range {
        start: range.start.map(cast_bound).transpose()?,
        end: range.end.map(cast_bound).transpose()?,
    })
}

pub(super) fn expr_of_i64(number: i64) -> sql_ast::Expr {
    sql_ast::Expr::Value(Value::Number(
        number.to_string(),
        number.leading_zeros() < 32,
    ))
}

pub(super) fn top_of_i64(take: i64, ctx: &mut Context) -> Top {
    let kind = ExprKind::Literal(Literal::Integer(take));
    let expr = Expr { kind, span: None };
    Top {
        quantity: Some(translate_expr(expr, ctx).unwrap()),
        with_ties: false,
        percent: false,
    }
}

pub(super) fn translate_select_item(cid: CId, ctx: &mut Context) -> Result<SelectItem> {
    let expr = translate_cid(cid, ctx)?;

    let inferred_name = match &expr {
        // sql_ast::Expr::Identifier is used for s-strings
        sql_ast::Expr::CompoundIdentifier(parts) => parts.last().map(|p| &p.value),
        _ => None,
    }
    .filter(|n| *n != "*");

    let expected = ctx.anchor.column_names.get(&cid);

    if inferred_name != expected {
        // use expected name
        let ident = expected.cloned().unwrap_or_else(|| {
            // or use something that will not clash with other names
            ctx.anchor.col_name.gen()
        });
        ctx.anchor.column_names.insert(cid, ident.to_string());

        return Ok(SelectItem::ExprWithAlias {
            alias: translate_ident_part(ident, ctx),
            expr,
        });
    }

    Ok(SelectItem::UnnamedExpr(expr))
}

/// Translates expr into IS NULL if possible, otherwise returns the expr unchanged.
///
/// Outer Result contains an error, inner Result contains the unmatched expr.
fn try_into_is_null(expr: Expr, ctx: &mut Context) -> Result<Result<sql_ast::Expr, Expr>> {
    let Some((decl, [a, b])) = try_unpack(&expr, [STD_EQ, STD_NE])? else {
        return Ok(Err(expr))
    };

    let take_a = if matches!(a.kind, ExprKind::Literal(Literal::Null)) {
        false
    } else if matches!(b.kind, ExprKind::Literal(Literal::Null)) {
        true
    } else {
        return Ok(Err(expr));
    };
    let is_std_eq = decl == STD_EQ;

    // we are sure this translates to IS NULL
    let [a, b] = unpack(expr, decl);
    let operand = if take_a { a } else { b };

    let strength =
        sql_ast::Expr::IsNull(Box::new(sql_ast::Expr::Value(Value::Null))).binding_strength();
    let expr = translate_operand(operand, strength, false, ctx)?;

    Ok(Ok(if is_std_eq {
        sql_ast::Expr::IsNull(expr)
    } else {
        sql_ast::Expr::IsNotNull(expr)
    }))
}

/// Translate expr into a BETWEEN statement if possible, otherwise returns the expr unchanged.
///
/// Outer Result contains an error, inner Result contains the unmatched expr.
fn try_into_between(expr: Expr, ctx: &mut Context) -> Result<Result<sql_ast::Expr, Expr>> {
    // validate that this expr matches the criteria

    let Some((_, [a, b])) = try_unpack(&expr, [STD_AND])? else {
        return Ok(Err(expr));
    };

    let Some((_, [a_l, _a_r])) = try_unpack(a, [STD_GTE])? else {
        return Ok(Err(expr));
    };
    let Some((_, [b_l, _b_r])) = try_unpack(b, [STD_LTE])? else {
        return Ok(Err(expr));
    };

    if a_l != b_l {
        return Ok(Err(expr));
    }

    // at this point we are sure that this should translate to Between
    // so the expr can be unpacked for good

    let [a, b] = unpack(expr, STD_AND);
    let [a_l, a_r] = unpack(a, STD_GTE);
    let [_, b_r] = unpack(b, STD_LTE);

    Ok(Ok(sql_ast::Expr::Between {
        expr: translate_operand(a_l, 0, false, ctx)?,
        negated: false,
        low: translate_operand(a_r, 0, false, ctx)?,
        high: translate_operand(b_r, 0, false, ctx)?,
    }))
}

fn translate_windowed(
    expr: sql_ast::Expr,
    window: Window,
    ctx: &mut Context,
    span: Option<Span>,
) -> Result<sql_ast::Expr> {
    let default_frame = {
        let (kind, range) = if window.sort.is_empty() {
            (WindowKind::Rows, Range::unbounded())
        } else {
            (
                WindowKind::Range,
                Range {
                    start: None,
                    end: Some(Expr {
                        kind: ExprKind::Literal(Literal::Integer(0)),
                        span: None,
                    }),
                },
            )
        };
        WindowFrame { kind, range }
    };

    let window = WindowSpec {
        partition_by: try_into_exprs(window.partition, ctx, span)?,
        order_by: (window.sort)
            .into_iter()
            .map(|sort| translate_column_sort(&sort, ctx))
            .try_collect()?,
        window_frame: if window.frame == default_frame {
            None
        } else {
            Some(try_into_window_frame(window.frame)?)
        },
    };

    Ok(sql_ast::Expr::Identifier(sql_ast::Ident::new(format!(
        "{expr} OVER ({window})"
    ))))
}

fn try_into_window_frame(frame: WindowFrame<Expr>) -> Result<sql_ast::WindowFrame> {
    fn parse_bound(bound: Expr) -> Result<WindowFrameBound> {
        let as_int = bound.kind.into_literal()?.into_integer()?;
        Ok(match as_int {
            0 => WindowFrameBound::CurrentRow,
            1.. => WindowFrameBound::Following(Some(Box::new(sql_ast::Expr::Value(
                sql_ast::Value::Number(as_int.to_string(), false),
            )))),
            _ => WindowFrameBound::Preceding(Some(Box::new(sql_ast::Expr::Value(
                sql_ast::Value::Number((-as_int).to_string(), false),
            )))),
        })
    }

    Ok(sql_ast::WindowFrame {
        units: match frame.kind {
            WindowKind::Rows => sql_ast::WindowFrameUnits::Rows,
            WindowKind::Range => sql_ast::WindowFrameUnits::Range,
        },
        start_bound: if let Some(start) = frame.range.start {
            parse_bound(start)?
        } else {
            WindowFrameBound::Preceding(None)
        },
        end_bound: Some(if let Some(end) = frame.range.end {
            parse_bound(end)?
        } else {
            WindowFrameBound::Following(None)
        }),
    })
}

pub(super) fn translate_column_sort(
    sort: &ColumnSort<CId>,
    ctx: &mut Context,
) -> Result<OrderByExpr> {
    Ok(OrderByExpr {
        expr: translate_cid(sort.column, ctx)?,
        asc: if matches!(sort.direction, SortDirection::Asc) {
            None // default order is ASC, so there is no need to emit it
        } else {
            Some(false)
        },
        nulls_first: None,
    })
}

/// Translate a PRQL Ident to a Vec of SQL Idents.
// We return a vec of SQL Idents because sqlparser sometimes uses
// [ObjectName](sql_ast::ObjectName) and sometimes uses
// [sql_ast::Expr::CompoundIdentifier](sql_ast::Expr::CompoundIdentifier), each of which
// contains `Vec<Ident>`.
pub(super) fn translate_ident(
    table_name: Option<String>,
    column: Option<String>,
    ctx: &Context,
) -> Vec<sql_ast::Ident> {
    let mut parts = Vec::with_capacity(4);
    if !ctx.query.omit_ident_prefix || column.is_none() {
        if let Some(table) = table_name {
            #[allow(clippy::if_same_then_else)]
            if ctx.dialect.big_query_quoting() {
                // Special-case this for BigQuery, Ref #852
                parts.push(table);
            } else if table.contains('*') {
                // This messy and could be cleaned up a lot.
                // If `parts` is (includung the backticks)
                //
                //   `schema.table`
                //
                // then we want to split it up, because we need to produce the
                // result without the surrounding backticks, ref #822.
                //
                // But if it's `path/*.parquet`, then we want to retain the
                // backticks.
                //
                // So for the moment, we check whether there's a `*` in there,
                // and if there is, we don't split it up.
                //
                // I think probably we should interpret `schema.table` as a
                // namespace when it's passed to `from` or `join`, but that
                // requires handling the types in those transforms.
                parts.push(table);
            } else {
                parts.extend(table.split('.').map(|s| s.to_string()));
            }
        }
    }

    parts.extend(column);

    parts
        .into_iter()
        .map(|x| translate_ident_part(x, ctx))
        .collect()
}

fn is_keyword(ident: &str) -> bool {
    lazy_static! {
        /// Keywords which we want to quote when translating to SQL. Currently we're
        /// being fairly permissive (over-quoting is not a concern), though we don't
        /// use `ALL_KEYWORDS`, which is quite broad, including words like `temp`
        /// and `lower`.
        static ref PRQL_KEYWORDS: HashSet<&'static Keyword> = {
            let mut m = HashSet::new();
            m.extend(RESERVED_FOR_COLUMN_ALIAS);
            m.extend(RESERVED_FOR_TABLE_ALIAS);
            m
        };
    }

    // Search for the ident in `ALL_KEYWORDS`, and then look it up in
    // `ALL_KEYWORDS_INDEX`. There doesn't seem to a simpler
    // `Keyword::from_string` function.
    let keyword = ALL_KEYWORDS
        .binary_search(&ident.to_ascii_uppercase().as_str())
        .map_or(Keyword::NoKeyword, |x| ALL_KEYWORDS_INDEX[x]);

    PRQL_KEYWORDS.contains(&keyword)
}

pub(super) fn translate_ident_part(ident: String, ctx: &Context) -> sql_ast::Ident {
    lazy_static! {
        // One of:
        // - `*`
        // - An ident starting with `a-z_\$` and containing other characters `a-z0-9_\$`
        //
        // We could replace this with pomsky (regex<>pomsky : sql<>prql)
        // ^ ('*' | [ascii_lower '_$'] [ascii_lower ascii_digit '_$']* ) $
        static ref VALID_BARE_IDENT: Regex = Regex::new(r"^((\*)|(^[a-z_\$][a-z0-9_\$]*))$").unwrap();
    }

    let is_bare = VALID_BARE_IDENT.is_match(&ident);

    if is_bare && !is_keyword(&ident) {
        sql_ast::Ident::new(ident)
    } else {
        sql_ast::Ident::with_quote(ctx.dialect.ident_quote(), ident)
    }
}

/// Wraps into parenthesis if binding strength would be less than min_strength
fn translate_operand(
    expr: Expr,
    parent_strength: i32,
    fix_associativity: bool,
    context: &mut Context,
) -> Result<Box<sql_ast::Expr>> {
    let expr = Box::new(translate_expr(expr, context)?);

    let strength = expr.binding_strength();

    // Either the binding strength is less than its parent, or it's equal and we
    // need to correct for the associativity of the operator (e.g. `a - (b - c)`)
    let needs_nesting =
        strength < parent_strength || (strength == parent_strength && fix_associativity);

    Ok(if needs_nesting {
        Box::new(sql_ast::Expr::Nested(expr))
    } else {
        expr
    })
}

/// Associativity of an expression's operator.
/// Note that there's no exponent symbol in SQL, so we don't seem to require a `Right` variant.
/// https://en.wikipedia.org/wiki/Operator_associativity
#[allow(dead_code)]
pub enum Associativity {
    Left,
    Both,
    Right,
}

trait SQLExpression {
    /// Returns binding strength of an SQL expression
    /// https://www.postgresql.org/docs/14/sql-syntax-lexical.html#id-1.5.3.5.13.2
    /// https://docs.microsoft.com/en-us/sql/t-sql/language-elements/operator-precedence-transact-sql?view=sql-server-ver16
    fn binding_strength(&self) -> i32;

    fn associativity(&self) -> Associativity {
        Associativity::Both
    }

    /// Returns true iff `a + b + c = (a + b) + c`
    fn associates_left(&self) -> bool {
        matches!(
            self.associativity(),
            Associativity::Left | Associativity::Both
        )
    }
    /// Returns true iff `a + b + c = a + (b + c)`
    fn associates_right(&self) -> bool {
        matches!(
            self.associativity(),
            Associativity::Right | Associativity::Both
        )
    }
}
impl SQLExpression for sql_ast::Expr {
    fn binding_strength(&self) -> i32 {
        // Strength of an expression depends only on the top-level operator, because all
        // other nested expressions can only have lower strength
        match self {
            sql_ast::Expr::BinaryOp { op, .. } => op.binding_strength(),

            sql_ast::Expr::UnaryOp { op, .. } => op.binding_strength(),

            sql_ast::Expr::Like { .. } | sql_ast::Expr::ILike { .. } => 7,

            sql_ast::Expr::IsNull(_) | sql_ast::Expr::IsNotNull(_) => 5,

            // all other items types bind stronger (function calls, literals, ...)
            _ => 20,
        }
    }
    fn associativity(&self) -> Associativity {
        match self {
            sql_ast::Expr::BinaryOp { op, .. } => op.associativity(),
            sql_ast::Expr::UnaryOp { op, .. } => op.associativity(),
            _ => Associativity::Both,
        }
    }
}
impl SQLExpression for BinaryOperator {
    fn binding_strength(&self) -> i32 {
        use BinaryOperator::*;
        match self {
            Modulo | Multiply | Divide => 11,
            Minus | Plus => 10,

            Gt | Lt | GtEq | LtEq | Eq | NotEq => 6,

            And => 3,
            Or => 2,

            _ => 9,
        }
    }
    fn associativity(&self) -> Associativity {
        use BinaryOperator::*;
        match self {
            Minus | Divide => Associativity::Left,
            _ => Associativity::Both,
        }
    }
}
impl SQLExpression for UnaryOperator {
    fn binding_strength(&self) -> i32 {
        match self {
            UnaryOperator::Minus | UnaryOperator::Plus => 13,
            UnaryOperator::Not => 4,
            _ => 9,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ast::pl::Range;
    use insta::assert_yaml_snapshot;

    #[test]
    fn test_range_of_ranges() -> Result<()> {
        fn from_ints(start: Option<i64>, end: Option<i64>) -> Range<Expr> {
            let start = start.map(|x| Expr {
                kind: ExprKind::Literal(Literal::Integer(x)),
                span: None,
            });
            let end = end.map(|x| Expr {
                kind: ExprKind::Literal(Literal::Integer(x)),
                span: None,
            });
            Range { start, end }
        }

        let range1 = from_ints(Some(1), Some(10));
        let range2 = from_ints(Some(5), Some(6));
        let range3 = from_ints(Some(5), None);
        let range4 = from_ints(None, Some(8));
        let range5 = from_ints(Some(5), Some(5));

        assert!(range_of_ranges(vec![range1.clone()])?.end.is_some());

        assert_yaml_snapshot!(range_of_ranges(vec![range1.clone()])?, @r###"
        ---
        start: 1
        end: 10
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range1.clone(), range1.clone()])?, @r###"
        ---
        start: 1
        end: 10
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range1.clone(), range2.clone()])?, @r###"
        ---
        start: 5
        end: 6
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range2.clone(), range1.clone()])?, @r###"
        ---
        start: 5
        end: 6
        "###);

        // empty range
        assert_yaml_snapshot!(range_of_ranges(vec![range2.clone(), range2.clone()])?, @r###"
        ---
        start: ~
        end: 0
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range3.clone(), range3.clone()])?, @r###"
        ---
        start: 9
        end: ~
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range1, range3])?, @r###"
        ---
        start: 5
        end: 10
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range2, range4.clone()])?, @r###"
        ---
        start: 5
        end: 6
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range4.clone(), range4])?, @r###"
        ---
        start: ~
        end: 8
        "###);

        assert_yaml_snapshot!(range_of_ranges(vec![range5])?, @r###"
        ---
        start: 5
        end: 5
        "###);

        Ok(())
    }

    #[test]
    fn test_translate_datetime_literal_with_sqlite_function() -> Result<()> {
        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Date,
                "2020-01-01".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: DATE
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: 2020-01-01
  over: ~
  distinct: false
  special: false
"###
        );

        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Time(None, sql_ast::TimezoneInfo::None),
                "03:05".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: TIME
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: "03:05"
  over: ~
  distinct: false
  special: false
"###
        );

        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Time(None, sql_ast::TimezoneInfo::None),
                "03:05+08:00".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: TIME
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: "03:05+08:00"
  over: ~
  distinct: false
  special: false
"###
        );

        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Time(None, sql_ast::TimezoneInfo::None),
                "03:05+0800".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: TIME
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: "03:05+08:00"
  over: ~
  distinct: false
  special: false
"###
        );

        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Timestamp(None, sql_ast::TimezoneInfo::None),
                "2021-03-14T03:05+0800".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: DATETIME
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: "2021-03-14T03:05+08:00"
  over: ~
  distinct: false
  special: false
"###
        );

        assert_yaml_snapshot!(
                translate_datetime_literal_with_sqlite_function(
                sql_ast::DataType::Timestamp(None, sql_ast::TimezoneInfo::None),
                "2021-03-14T03:05+08:00".to_string(),
            ),
            @r###"
---
Function:
  name:
    - value: DATETIME
      quote_style: ~
  args:
    - Unnamed:
        Expr:
          Value:
            SingleQuotedString: "2021-03-14T03:05+08:00"
  over: ~
  distinct: false
  special: false
"###
        );

        Ok(())
    }
}
