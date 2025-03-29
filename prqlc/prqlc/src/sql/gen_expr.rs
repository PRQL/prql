//! Contains functions that compile [crate::pr::pl] nodes into [sqlparser] nodes.

use std::cmp::Ordering;

use itertools::Itertools;
use prqlc_parser::generic::{InterpolateItem, Range};
use regex::Regex;
use sqlparser::ast::{
    self as sql_ast, BinaryOperator, DateTimeField, Fetch, Function, FunctionArg, FunctionArgExpr,
    FunctionArgumentList, ObjectName, OrderByExpr, SelectItem, UnaryOperator, Value,
    WindowFrameBound, WindowSpec,
};

use super::gen_projection::try_into_exprs;
use super::{keywords, Context};
use crate::ir::generic::{ColumnSort, SortDirection, WindowFrame, WindowKind};
use crate::ir::pl::{self, Ident, Literal};
use crate::ir::rq;
use crate::sql::pq::context::ColumnDecl;
use crate::utils::{valid_ident, OrMap};
use crate::{Error, Reason, Result, Span, WithErrorInfo};

pub(super) fn translate_expr(expr: rq::Expr, ctx: &mut Context) -> Result<ExprOrSource> {
    Ok(match expr.kind {
        rq::ExprKind::ColumnRef(cid) => translate_cid(cid, ctx)?,

        // Fairly hacky — convert everything to a string, then concat it,
        // then convert to sql_ast::Expr. We can't use the `Item::sql_ast::Expr` code above
        // since we don't want to intersperse with spaces.
        rq::ExprKind::SString(s_string_items) => {
            let text = translate_sstring(s_string_items, ctx)?;

            ExprOrSource::Source(SourceExpr {
                text,
                binding_strength: 100,
                window_frame: false,
            })
        }
        rq::ExprKind::Param(id) => ExprOrSource::Source(SourceExpr {
            text: format!("${id}"),
            binding_strength: 100,
            window_frame: false,
        }),
        rq::ExprKind::Literal(l) => translate_literal(l, ctx)?.into(),
        rq::ExprKind::Case(mut cases) => {
            let default = cases
                .last()
                .filter(|last| {
                    matches!(
                        last.condition.kind,
                        rq::ExprKind::Literal(Literal::Boolean(true))
                    )
                })
                .map(|def| translate_expr(def.value.clone(), ctx))
                .transpose()?
                .map(|x| x.into_ast());

            if default.is_some() {
                cases.pop();
            }

            let else_result = default
                .or(Some(sql_ast::Expr::Value(Value::Null.into())))
                .map(Box::new);

            let conditions = cases
                .into_iter()
                .map(|case| -> Result<_> {
                    let condition = translate_expr(case.condition, ctx)?.into_ast();
                    let result = translate_expr(case.value, ctx)?.into_ast();
                    Ok(sql_ast::CaseWhen { condition, result })
                })
                .try_collect()?;

            sql_ast::Expr::Case {
                operand: None,
                conditions,
                else_result,
            }
            .into()
        }
        rq::ExprKind::Operator { ref name, ref args } => {
            // A few special cases and then fall-through to the standard approach.
            match name.as_str() {
                // See notes in `std.rs` re whether we use names vs.
                // `FunctionDecl` vs. an Enum; and getting the correct
                // number of args from there. Currently the error messages
                // for the wrong number of args will be bad (though it's an
                // unusual case where RQ contains something like `std.eq`
                // with the wrong number of args).
                "std.eq" | "std.ne" => {
                    if let [a, b] = args.as_slice() {
                        if a.kind == rq::ExprKind::Literal(Literal::Null)
                            || b.kind == rq::ExprKind::Literal(Literal::Null)
                        {
                            return Ok(process_null(name, args, ctx)?.into());
                        } else {
                            let op = operator_from_name(name).unwrap();
                            return Ok(translate_binary_operator(a, b, op, ctx)?.into());
                        }
                    }
                }
                "std.concat" => return Ok(process_concat(&expr, ctx)?.into()),
                "std.array_in" => return Ok(process_array_in(&expr, args, ctx)?.into()),
                "std.date.to_text" => {
                    return Ok(process_date_to_text(&expr, name, args, ctx)?.into())
                }
                _ => match try_into_between(expr.clone(), ctx)? {
                    Some(between_expr) => return Ok(between_expr.into()),
                    None => {
                        if let Some(op) = operator_from_name(name) {
                            if let [left, right] = args.as_slice() {
                                return Ok(translate_binary_operator(left, right, op, ctx)?.into());
                            }
                        }
                    }
                },
            }
            super::operators::translate_operator_expr(expr, ctx)?
        }
        rq::ExprKind::Array(_) => {
            return Err(Error::new(Reason::Unexpected {
                found: "array of values (not supported here)".to_string(),
            })
            .with_span(expr.span));
        }
    })
}

/// Translates into IS NULL if possible
fn process_null(name: &str, args: &[rq::Expr], ctx: &mut Context) -> Result<sql_ast::Expr> {
    let (a, b) = (&args[0], &args[1]);
    let operand = if matches!(a.kind, rq::ExprKind::Literal(Literal::Null)) {
        b
    } else {
        a
    };

    // If this were an Enum, we could match on it (see notes in `std.rs`).
    if name == "std.eq" {
        let strength =
            sql_ast::Expr::IsNull(Box::new(sql_ast::Expr::Value(Value::Null.into()))).binding_strength();
        let expr = translate_operand(operand.clone(), true, strength, Associativity::Both, ctx)?;
        let expr = Box::new(expr.into_ast());
        Ok(sql_ast::Expr::IsNull(expr))
    } else if name == "std.ne" {
        let strength = sql_ast::Expr::IsNotNull(Box::new(sql_ast::Expr::Value(Value::Null.into())))
            .binding_strength();
        let expr = translate_operand(operand.clone(), true, strength, Associativity::Both, ctx)?;
        let expr = Box::new(expr.into_ast());
        Ok(sql_ast::Expr::IsNotNull(expr))
    } else {
        unreachable!()
    }
}

/// Translates into IN (v1, v2, ...) if possible
fn process_array_in(
    expr: &rq::Expr,
    args: &[rq::Expr],
    ctx: &mut Context,
) -> Result<sql_ast::Expr> {
    match args {
        [col_expr @ rq::Expr {
            kind:
                rq::ExprKind::ColumnRef(_)
                | rq::ExprKind::Literal(_)
                | rq::ExprKind::SString(_)
                | rq::ExprKind::Param(_)
                | rq::ExprKind::Operator { name: _, args: _ },
            ..
        }, rq::Expr {
            kind: rq::ExprKind::Array(in_values),
            ..
        }] => {
            if in_values.is_empty() {
                // We avoid producing `in ()` expressions since they are not syntactically valid
                // in some engines like PostgreSQL or MySQL.
                // We can instead optimize this to a condition that is always false
                Ok(sql_ast::Expr::Value(Value::Boolean(false).into()))
            } else {
                Ok(sql_ast::Expr::InList {
                    expr: Box::new(translate_expr(col_expr.clone(), ctx)?.into_ast()),
                    list: in_values
                        .iter()
                        .map(|a| Ok(translate_expr(a.clone(), ctx)?.into_ast()))
                        .collect::<Result<Vec<sql_ast::Expr>>>()?,
                    negated: false,
                })
            }
        }
        _ => Err(
            Error::new_simple("args to `std.array_in` must be an expression and an array")
                .with_span(expr.span),
        ),
    }
}

/// Translates PRQL date format (based on `chrono` crate) to dialect specific date format
/// For now only date format as string literal is supported
fn process_date_to_text(
    expr: &rq::Expr,
    op_name: &str,
    args: &[rq::Expr],
    ctx: &mut Context,
) -> Result<sql_ast::Expr> {
    if let [date_format_exp @ rq::Expr {
        kind: rq::ExprKind::Literal(Literal::String(date_format)),
        ..
    }, col_expr] = args
    {
        let expr = rq::Expr {
            kind: rq::ExprKind::Operator {
                name: op_name.to_string(),
                args: vec![
                    rq::Expr {
                        kind: rq::ExprKind::Literal(Literal::String(
                            ctx.dialect
                                .translate_prql_date_format(date_format)
                                .map_err(|e| e.with_span(date_format_exp.span))?,
                        )),
                        span: date_format_exp.span,
                    },
                    col_expr.clone(),
                ],
            },
            ..expr.clone()
        };
        Ok(super::operators::translate_operator_expr(expr, ctx)?.into_ast())
    } else {
        Err(
            Error::new_simple("`std.date.to_text` only supports a string literal as format")
                .with_span(expr.span),
        )
    }
}

fn process_concat(expr: &rq::Expr, ctx: &mut Context) -> Result<sql_ast::Expr> {
    if ctx.dialect.has_concat_function() {
        let concat_args = collect_concat_args(expr);

        let args_list = concat_args
            .iter()
            .map(|a| {
                translate_expr((*a).clone(), ctx)
                    .map(|x| FunctionArg::Unnamed(FunctionArgExpr::Expr(x.into_ast())))
            })
            .try_collect()?;

        let args = sql_ast::FunctionArguments::List(FunctionArgumentList {
            args: args_list,
            clauses: vec![],
            duplicate_treatment: None,
        });

        Ok(sql_ast::Expr::Function(Function {
            name: ObjectName(vec![sqlparser::ast::ObjectNamePart::Identifier(sql_ast::Ident::new("CONCAT"))]),
            args,
            over: None,
            filter: None,
            null_treatment: None,
            within_group: vec![],
            parameters: sql_ast::FunctionArguments::None,
            uses_odbc_syntax: false,
        }))
    } else {
        let concat_args = collect_concat_args(expr);

        let mut iter = concat_args.into_iter();
        let first_expr = iter.next().unwrap();
        let mut current_expr = translate_expr(first_expr.clone(), ctx)?.into_ast();

        for arg in iter {
            let translated_arg = translate_expr(arg.clone(), ctx)?.into_ast();
            current_expr = sql_ast::Expr::BinaryOp {
                left: Box::new(current_expr),
                op: BinaryOperator::StringConcat,
                right: Box::new(translated_arg),
            };
        }

        Ok(current_expr)
    }
}

fn translate_binary_operator(
    left: &rq::Expr,
    right: &rq::Expr,
    op: BinaryOperator,
    ctx: &mut Context,
) -> Result<sql_ast::Expr> {
    let strength = op.binding_strength();

    let left = translate_operand(left.clone(), true, strength, op.associativity(), ctx)?;
    let right = translate_operand(right.clone(), false, strength, op.associativity(), ctx)?;

    let left = Box::new(left.into_ast());
    let right = Box::new(right.into_ast());

    Ok(sql_ast::Expr::BinaryOp { left, op, right })
}

fn collect_concat_args(expr: &rq::Expr) -> Vec<&rq::Expr> {
    match &expr.kind {
        rq::ExprKind::Operator { name, args } if name == "std.concat" => {
            args.iter().flat_map(collect_concat_args).collect()
        }
        _ => vec![expr],
    }
}

/// Translate expr into a BETWEEN statement if possible, otherwise returns the expr unchanged.
fn try_into_between(expr: rq::Expr, ctx: &mut Context) -> Result<Option<sql_ast::Expr>> {
    match expr.kind {
        rq::ExprKind::Operator { name, args } if name == "std.and" => {
            let [a, b]: [_; 2] = args.try_into().unwrap();

            match (a.kind, b.kind) {
                (
                    rq::ExprKind::Operator {
                        name: a_name,
                        args: a_args,
                    },
                    rq::ExprKind::Operator {
                        name: b_name,
                        args: b_args,
                    },
                ) if a_name == "std.gte" && b_name == "std.lte" => {
                    let [a_l, a_r]: [_; 2] = a_args.try_into().unwrap();
                    let [b_l, b_r]: [_; 2] = b_args.try_into().unwrap();

                    // We need for the values on each arm to be the same; e.g. x
                    // > 3 and x < 5
                    if a_l == b_l {
                        return Ok(Some(sql_ast::Expr::Between {
                            expr: Box::new(
                                translate_operand(a_l, true, 0, Associativity::Both, ctx)?
                                    .into_ast(),
                            ),
                            negated: false,
                            low: Box::new(
                                translate_operand(a_r, true, 0, Associativity::Both, ctx)?
                                    .into_ast(),
                            ),
                            high: Box::new(
                                translate_operand(b_r, true, 0, Associativity::Both, ctx)?
                                    .into_ast(),
                            ),
                        }));
                    }
                }
                _ => (),
            }
        }
        _ => (),
    }
    Ok(None)
}

fn operator_from_name(name: &str) -> Option<BinaryOperator> {
    use BinaryOperator::*;
    match name {
        "std.mul" => Some(Multiply),
        "std.add" => Some(Plus),
        "std.sub" => Some(Minus),
        "std.eq" => Some(Eq),
        "std.ne" => Some(NotEq),
        "std.gt" => Some(Gt),
        "std.lt" => Some(Lt),
        "std.gte" => Some(GtEq),
        "std.lte" => Some(LtEq),
        "std.and" => Some(And),
        "std.or" => Some(Or),
        "std.concat" => Some(StringConcat),
        _ => None,
    }
}

pub(super) fn translate_literal(l: Literal, ctx: &Context) -> Result<sql_ast::Expr> {
    Ok(match l {
        Literal::Null => sql_ast::Expr::Value(Value::Null.into()),
        Literal::String(s) | Literal::RawString(s) => {
            sql_ast::Expr::Value(Value::SingleQuotedString(s).into())
        }
        Literal::Boolean(b) => sql_ast::Expr::Value(Value::Boolean(b).into()),
        Literal::Float(f) => sql_ast::Expr::Value(Value::Number(format!("{f:?}"), false).into()),
        Literal::Integer(i) => sql_ast::Expr::Value(Value::Number(format!("{i}"), false).into()),
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
                "weeks" => DateTimeField::Week(None),
                "days" => DateTimeField::Day,
                "hours" => DateTimeField::Hour,
                "minutes" => DateTimeField::Minute,
                "seconds" => DateTimeField::Second,
                "milliseconds" => DateTimeField::Millisecond,
                "microseconds" => DateTimeField::Microsecond,
                _ => {
                    return Err(Error::new_simple(format!(
                        "Unsupported interval unit: {}",
                        vau.unit
                    )))
                }
            };
            if ctx.dialect.requires_quotes_intervals() {
                //postgres requires quotes around number and unit together eg '3 WEEK'
                let value = Box::new(sql_ast::Expr::Value(Value::SingleQuotedString(format!(
                    "{} {}",
                    vau.n, sql_parser_datetime
                )).into()));
                sql_ast::Expr::Interval(sqlparser::ast::Interval {
                    value,
                    leading_field: None, //set to none since field is now contained in string
                    leading_precision: None,
                    last_field: None,
                    fractional_seconds_precision: None,
                })
            } else {
                let value = Box::new(translate_literal(Literal::Integer(vau.n), ctx)?);
                sql_ast::Expr::Interval(sqlparser::ast::Interval {
                    value,
                    leading_field: Some(sql_parser_datetime),
                    leading_precision: None,
                    last_field: None,
                    fractional_seconds_precision: None,
                })
            }
        }
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
    sql_ast::Expr::TypedString { data_type, value: sqlparser::ast::Value::SingleQuotedString(value) }
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
        Value::SingleQuotedString(time_value).into(),
    )));

    let func_name = match data_type {
        sql_ast::DataType::Date => data_type.to_string(),
        sql_ast::DataType::Time(..) => data_type.to_string(),
        sql_ast::DataType::Timestamp(..) => "DATETIME".to_string(),
        _ => unreachable!(),
    };

    let args = sql_ast::FunctionArguments::List(FunctionArgumentList {
        args: vec![arg],
        clauses: vec![],
        duplicate_treatment: None,
    });

    sql_ast::Expr::Function(Function {
        name: ObjectName(vec![sqlparser::ast::ObjectNamePart::Identifier(sql_ast::Ident::new(func_name))]),
        args,
        over: None,
        filter: None,
        null_treatment: None,
        within_group: vec![],
        parameters: sql_ast::FunctionArguments::None,
        uses_odbc_syntax: false,
    })
}

pub(super) fn translate_cid(cid: rq::CId, ctx: &mut Context) -> Result<ExprOrSource> {
    if ctx.query.pre_projection {
        log::debug!("translating {cid:?} pre projection");
        let decl = ctx.anchor.column_decls.get(&cid).expect("bad RQ ids");

        Ok(match decl {
            ColumnDecl::Compute(compute) => {
                let window = compute.window.clone();
                let span = compute.expr.span;

                let prev_wf = ctx.query.window_function;
                ctx.query.window_function = window.is_some();
                let expr = translate_expr(compute.expr.clone(), ctx)?;
                ctx.query.window_function = prev_wf;

                if let Some(window) = window {
                    translate_windowed(expr, window, ctx, span)?
                } else {
                    expr
                }
            }
            ColumnDecl::RelationColumn(riid, _, col) => {
                let column = match col.clone() {
                    rq::RelationColumn::Wildcard => translate_star(ctx, None)?,
                    rq::RelationColumn::Single(name) => name.unwrap(),
                };
                let t = &ctx.anchor.relation_instances[riid];

                let table_ident = t.table_ref.name.clone().map(Ident::from_name);
                let ident = translate_ident(table_ident, Some(column), ctx);
                sql_ast::Expr::CompoundIdentifier(ident).into()
            }
        })
    } else {
        // translate into ident
        let column_decl = &&ctx.anchor.column_decls[&cid];

        let table_name = if let ColumnDecl::RelationColumn(riid, _, _) = column_decl {
            let t = &ctx.anchor.relation_instances[riid];
            Some(t.table_ref.name.clone().unwrap())
        } else {
            None
        };

        let column = match &column_decl {
            ColumnDecl::RelationColumn(_, _, rq::RelationColumn::Wildcard) => {
                translate_star(ctx, None)?
            }

            _ => {
                let name = ctx.anchor.column_names.get(&cid).cloned();
                name.expect("name of this column has not been to be set before generating SQL")
            }
        };

        let ident = translate_ident(table_name.map(Ident::from_name), Some(column), ctx);

        log::debug!("translating {cid:?} post projection: {ident:?}");

        let ident = sql_ast::Expr::CompoundIdentifier(ident);
        Ok(ident.into())
    }
}

pub(super) fn translate_star(ctx: &Context, span: Option<Span>) -> Result<String> {
    if !ctx.query.allow_stars {
        Err(
            Error::new_simple("Target dialect does not support * in this position.")
                .with_span(span),
        )
    } else {
        Ok("*".to_string())
    }
}

pub(super) fn translate_sstring(
    items: Vec<InterpolateItem<rq::Expr>>,
    ctx: &mut Context,
) -> Result<String> {
    Ok(items
        .into_iter()
        .map(|s_string_item| match s_string_item {
            InterpolateItem::String(string) => Ok(string),
            InterpolateItem::Expr { expr, .. } => {
                translate_expr(*expr, ctx).map(|expr| expr.into_source())
            }
        })
        .collect::<Result<Vec<String>>>()?
        .join(""))
}

/// Aggregate several ordered ranges into one, computing the intersection.
///
/// Returns a tuple of `(start, end)`, where `end` is optional.
pub(super) fn range_of_ranges(ranges: Vec<Range<rq::Expr>>) -> Result<Range<i64>> {
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

fn unpack_as_int_literal(bound: rq::Expr) -> Result<i64> {
    Some(bound.kind)
        .and_then(|x| x.into_literal().ok())
        .and_then(|x| x.into_integer().ok())
        .ok_or_else(|| Error::new_simple("expected an integer literal").with_span(bound.span))
}

fn try_range_into_int(range: Range<rq::Expr>) -> Result<Range<i64>> {
    Ok(Range {
        start: range.start.map(unpack_as_int_literal).transpose()?,
        end: range.end.map(unpack_as_int_literal).transpose()?,
    })
}

pub(super) fn expr_of_i64(number: i64) -> sql_ast::Expr {
    sql_ast::Expr::Value(Value::Number(
        number.to_string(),
        number.leading_zeros() < 32,
    ).into())
}

pub(super) fn fetch_of_i64(take: i64, ctx: &mut Context) -> Fetch {
    let kind = rq::ExprKind::Literal(Literal::Integer(take));
    let expr = rq::Expr { kind, span: None };
    Fetch {
        quantity: Some(translate_expr(expr, ctx).unwrap().into_ast()),
        with_ties: false,
        percent: false,
    }
}

pub(super) fn translate_select_item(cid: rq::CId, ctx: &mut Context) -> Result<SelectItem> {
    let expr = translate_cid(cid, ctx)?.into_ast();

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

fn translate_windowed(
    expr: ExprOrSource,
    window: rq::Window,
    ctx: &mut Context,
    span: Option<Span>,
) -> Result<ExprOrSource> {
    let default_frame = {
        let (kind, range) = if window.sort.is_empty() {
            (WindowKind::Rows, Range::unbounded())
        } else {
            (
                WindowKind::Range,
                Range {
                    start: None,
                    end: Some(rq::Expr {
                        kind: rq::ExprKind::Literal(Literal::Integer(0)),
                        span: None,
                    }),
                },
            )
        };
        WindowFrame { kind, range }
    };

    let supports_frame = matches!(
        expr,
        ExprOrSource::Source(SourceExpr {
            window_frame: true,
            ..
        })
    );

    let window = WindowSpec {
        window_name: None,
        partition_by: try_into_exprs(window.partition, ctx, span)?,
        order_by: (window.sort)
            .into_iter()
            .map(|sort| translate_column_sort(&sort, ctx))
            .try_collect()?,
        window_frame: if supports_frame && window.frame != default_frame {
            Some(try_into_window_frame(window.frame)?)
        } else {
            None
        },
    };

    let expr = expr.into_source();
    Ok(ExprOrSource::Source(SourceExpr {
        text: format!("{expr} OVER ({window})"),
        binding_strength: 100,
        window_frame: false,
    }))
}

fn try_into_window_frame(frame: WindowFrame<rq::Expr>) -> Result<sql_ast::WindowFrame> {
    fn parse_bound(bound: rq::Expr) -> Result<WindowFrameBound> {
        let as_int = unpack_as_int_literal(bound)?;
        Ok(match as_int {
            0 => WindowFrameBound::CurrentRow,
            1.. => WindowFrameBound::Following(Some(Box::new(sql_ast::Expr::Value(
                sql_ast::Value::Number(as_int.to_string(), false).into(),
            )))),
            _ => WindowFrameBound::Preceding(Some(Box::new(sql_ast::Expr::Value(
                sql_ast::Value::Number((-as_int).to_string(), false).into(),
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
    sort: &ColumnSort<rq::CId>,
    ctx: &mut Context,
) -> Result<OrderByExpr> {
    Ok(OrderByExpr {
        expr: translate_cid(sort.column, ctx)?.into_ast(),
        options: sqlparser::ast::OrderByOptions {
            asc: if matches!(sort.direction, SortDirection::Asc) {
                None // default order is ASC, so there is no need to emit it
            } else {
                Some(false)
            },
            nulls_first: None,
        },
        with_fill: None,
    })
}

/// Translate a PRQL Ident to a Vec of SQL Idents.
// We return a vec of SQL Idents because sqlparser sometimes uses
// [ObjectName](sql_ast::ObjectName) and sometimes uses
// [sql_ast::Expr::CompoundIdentifier](sql_ast::Expr::CompoundIdentifier), each of which
// contains `Vec<Ident>`.
pub(super) fn translate_ident(
    table_ident: Option<pl::Ident>,
    column: Option<String>,
    ctx: &Context,
) -> Vec<sql_ast::Ident> {
    let mut parts = Vec::with_capacity(4);
    if !ctx.query.omit_ident_prefix || column.is_none() {
        if let Some(table) = table_ident {
            parts.extend(table);
        }
    }

    parts.extend(column);

    parts
        .into_iter()
        .map(|x| translate_ident_part(x, ctx))
        .collect()
}

pub(super) fn translate_ident_part(ident: String, ctx: &Context) -> sql_ast::Ident {
    let is_bare = valid_ident().is_match(&ident);

    if is_bare && !keywords::is_keyword(&ident) {
        sql_ast::Ident::new(ident)
    } else {
        sql_ast::Ident::with_quote(ctx.dialect.ident_quote(), ident)
    }
}

pub(super) fn translate_operand(
    expr: rq::Expr,
    is_left: bool,
    parent_strength: i32,
    parent_associativity: Associativity,
    context: &mut Context,
) -> Result<ExprOrSource> {
    let expr = translate_expr(expr, context)?;

    if needs_parentheses(&expr, is_left, parent_strength, parent_associativity) {
        Ok(expr.wrap_in_parenthesis())
    } else {
        Ok(expr)
    }
}

/// For an operation represented as `a child b` with a surrounding parent
/// operation (e.g., `(a child b) parent c` or `a parent (b child c)`):
///
/// 1. When the child operator has higher precedence than the parent,
///    parentheses *are not* required.
///
/// 2. When the child operator has lower precedence than the parent,
///    parentheses *are* required.
///
/// 3. When the child and parent operators have the same precedence, the child
///    is on the {left,right} and the parent is {left,right} associative,
///    parentheses are not required. Some examples of when parentheses are not required:
///    - `(a - b) - c` & `(a + b) - c` — as opposed to `a - (b - c)`
///    - `a + (b - c)` & `a + (b + c)` — as opposed to `a - (b + c)` & `a - (b - c)`
///
///
//
// If it were possible to evaluate this with less context that would be
// preferable, but it's not clear how to do that. (For example, even if we
// passed a reference to the parent, that still wouldn't tell us whether the
// child were on the left or the right, which is required...)
//
// Note that the code is deliberately somewhat verbose. While it could instead
// be a neat single expression, it was quite difficult to work through, so
// please do not make the code terser without being confident that it's easier
// to understand.
fn needs_parentheses(
    expr: &ExprOrSource,
    is_left: bool,
    parent_strength: i32,
    parent_associativity: Associativity,
) -> bool {
    let rule_3a = matches!(parent_associativity, Associativity::Both);
    let rule_3b_left = is_left && parent_associativity.left_associative();
    let rule_3b_right = !is_left && parent_associativity.right_associative();

    match expr.binding_strength().cmp(&parent_strength) {
        // Rule 1
        Ordering::Greater => false,
        // Rule 2
        Ordering::Less => true,
        // Rule 3
        Ordering::Equal => !(rule_3a || rule_3b_left || rule_3b_right),
    }
}

/// Associativity of an expression's operator.
/// Note that there's no exponent symbol in SQL, so we don't seem to require a `Right` variant.
/// https://en.wikipedia.org/wiki/Operator_associativity
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq)]
pub enum Associativity {
    Left,
    /// `Both` means mathematically associative, like `+` or `*`
    Both,
    Right,
}

impl Associativity {
    /// Returns true iff `a + b + c = (a + b) + c`
    fn left_associative(&self) -> bool {
        matches!(self, Associativity::Left | Associativity::Both)
    }

    /// Returns true iff `a + b + c = a + (b + c)`
    fn right_associative(&self) -> bool {
        matches!(self, Associativity::Right | Associativity::Both)
    }
}

trait SQLExpression {
    /// Returns binding strength of a SQL expression
    /// https://www.postgresql.org/docs/14/sql-syntax-lexical.html#id-1.5.3.5.13.2
    /// https://docs.microsoft.com/en-us/sql/t-sql/language-elements/operator-precedence-transact-sql?view=sql-server-ver16
    fn binding_strength(&self) -> i32;

    /// Default to `Both`, but expected to be overwritten by concrete types
    fn associativity(&self) -> Associativity {
        Associativity::Both
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
            Minus | Divide | Modulo => Associativity::Left,
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

/// A wrapper around sql_ast::Expr, that may have already been converted to source.
#[derive(Debug, Clone)]
pub enum ExprOrSource {
    Expr(sql_ast::Expr),
    Source(SourceExpr),
}

#[derive(Debug, Clone)]
pub struct SourceExpr {
    pub text: String,
    pub binding_strength: i32,

    /// True for expressions that support (and need) the window frame (OVER ...)
    pub window_frame: bool,
}

impl ExprOrSource {
    pub fn into_ast(self) -> sql_ast::Expr {
        match self {
            ExprOrSource::Expr(ast) => ast,
            ExprOrSource::Source(SourceExpr { text: source, .. }) => {
                // The s-string hack
                sql_ast::Expr::Identifier(sql_ast::Ident::new(source))
            }
        }
    }

    pub fn into_source(self) -> String {
        match self {
            ExprOrSource::Expr(e) => e.to_string(),
            ExprOrSource::Source(SourceExpr { text, .. }) => text,
        }
    }

    fn wrap_in_parenthesis(self) -> Self {
        match self {
            ExprOrSource::Expr(expr) => ExprOrSource::Expr(sql_ast::Expr::Nested(Box::new(expr))),
            ExprOrSource::Source(SourceExpr {
                text, window_frame, ..
            }) => {
                let text = format!("({text})");
                ExprOrSource::Source(SourceExpr {
                    text,
                    binding_strength: 100,
                    window_frame,
                })
            }
        }
    }
}

impl SQLExpression for ExprOrSource {
    fn binding_strength(&self) -> i32 {
        match self {
            ExprOrSource::Expr(expr) => expr.binding_strength(),
            ExprOrSource::Source(SourceExpr {
                binding_strength, ..
            }) => *binding_strength,
        }
    }
}

impl From<sql_ast::Expr> for ExprOrSource {
    fn from(value: sql_ast::Expr) -> Self {
        ExprOrSource::Expr(value)
    }
}

#[cfg(test)]
mod test {
    use insta::assert_yaml_snapshot;

    use super::*;

    #[test]
    fn test_range_of_ranges() -> Result<()> {
        fn from_ints(start: Option<i64>, end: Option<i64>) -> Range<rq::Expr> {
            let start = start.map(|x| rq::Expr {
                kind: rq::ExprKind::Literal(Literal::Integer(x)),
                span: None,
            });
            let end = end.map(|x| rq::Expr {
                kind: rq::ExprKind::Literal(Literal::Integer(x)),
                span: None,
            });
            Range { start, end }
        }

        let range_1_10 = from_ints(Some(1), Some(10));
        let range_5_6 = from_ints(Some(5), Some(6));
        let range_5_inf = from_ints(Some(5), None);
        let range_inf_8 = from_ints(None, Some(8));
        let range_5_5 = from_ints(Some(5), Some(5));

        assert!(range_of_ranges(vec![range_1_10.clone()])?.end.is_some());

        assert_yaml_snapshot!(range_of_ranges(vec![range_1_10.clone()])?, @r"
        start: 1
        end: 10
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_1_10.clone(), range_1_10.clone()])?, @r"
        start: 1
        end: 10
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_1_10.clone(), range_5_6.clone()])?, @r"
        start: 5
        end: 6
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_5_6.clone(), range_1_10.clone()])?, @r"
        start: 5
        end: 6
        ");

        // empty range
        assert_yaml_snapshot!(range_of_ranges(vec![range_5_6.clone(), range_5_6.clone()])?, @r"
        start: ~
        end: 0
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_5_inf.clone(), range_5_inf.clone()])?, @r"
        start: 9
        end: ~
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_1_10, range_5_inf])?, @r"
        start: 5
        end: 10
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_5_6, range_inf_8.clone()])?, @r"
        start: 5
        end: 6
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_inf_8.clone(), range_inf_8])?, @r"
        start: ~
        end: 8
        ");

        assert_yaml_snapshot!(range_of_ranges(vec![range_5_5])?, @r"
        start: 5
        end: 5
        ");

        Ok(())
    }
}
