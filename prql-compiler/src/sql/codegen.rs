//! Contains functions that compile [crate::ast::pl] nodes into [sqlparser] nodes.

use anyhow::{bail, Result};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use sqlparser::ast::{
    self as sql_ast, BinaryOperator, DateTimeField, Function, FunctionArg, FunctionArgExpr, Ident,
    Join, JoinConstraint, JoinOperator, ObjectName, OrderByExpr, SelectItem, TableAlias,
    TableFactor, Top, UnaryOperator, Value, WindowFrameBound, WindowSpec,
};
use sqlparser::keywords::{
    Keyword, ALL_KEYWORDS, ALL_KEYWORDS_INDEX, RESERVED_FOR_COLUMN_ALIAS, RESERVED_FOR_TABLE_ALIAS,
};
use std::collections::HashSet;

use crate::ast::pl::{
    BinOp, ColumnSort, InterpolateItem, JoinSide, Literal, Range, SortDirection, WindowFrame,
    WindowKind,
};
use crate::ast::rq::*;
use crate::error::{Error, Reason};
use crate::sql::context::ColumnDecl;
use crate::utils::OrMap;

use super::translator::Context;
use super::Dialect;

pub(super) fn translate_expr_kind(item: ExprKind, ctx: &mut Context) -> Result<sql_ast::Expr> {
    Ok(match item {
        ExprKind::ColumnRef(cid) => translate_cid(cid, ctx)?,
        ExprKind::Binary { op, left, right } => {
            if let Some(is_null) = try_into_is_null(&op, &left, &right, ctx)? {
                is_null
            } else if let Some(between) = try_into_between(&op, &left, &right, ctx)? {
                between
            } else {
                let op = match op {
                    BinOp::Mul => BinaryOperator::Multiply,
                    BinOp::Div => BinaryOperator::Divide,
                    BinOp::Mod => BinaryOperator::Modulo,
                    BinOp::Add => BinaryOperator::Plus,
                    BinOp::Sub => BinaryOperator::Minus,
                    BinOp::Eq => BinaryOperator::Eq,
                    BinOp::Ne => BinaryOperator::NotEq,
                    BinOp::Gt => BinaryOperator::Gt,
                    BinOp::Lt => BinaryOperator::Lt,
                    BinOp::Gte => BinaryOperator::GtEq,
                    BinOp::Lte => BinaryOperator::LtEq,
                    BinOp::And => BinaryOperator::And,
                    BinOp::Or => BinaryOperator::Or,
                    BinOp::Coalesce => {
                        let left = translate_operand(left.kind, 0, false, ctx)?;
                        let right = translate_operand(right.kind, 0, false, ctx)?;

                        return Ok(sql_ast::Expr::Function(Function {
                            name: ObjectName(vec![Ident {
                                value: "COALESCE".to_string(),
                                quote_style: None,
                            }]),
                            args: vec![
                                FunctionArg::Unnamed(FunctionArgExpr::Expr(*left)),
                                FunctionArg::Unnamed(FunctionArgExpr::Expr(*right)),
                            ],
                            over: None,
                            distinct: false,
                            special: false,
                        }));
                    }
                };

                let strength = op.binding_strength();
                let left = translate_operand(left.kind, strength, !op.associates_left(), ctx)?;
                let right = translate_operand(right.kind, strength, !op.associates_right(), ctx)?;
                sql_ast::Expr::BinaryOp { left, right, op }
            }
        }

        ExprKind::Unary { op, expr } => {
            let op = match op {
                UnOp::Neg => UnaryOperator::Minus,
                UnOp::Not => UnaryOperator::Not,
            };
            let expr = translate_operand(expr.kind, op.binding_strength(), false, ctx)?;
            sql_ast::Expr::UnaryOp { op, expr }
        }

        // Fairly hacky — convert everything to a string, then concat it,
        // then convert to sql_ast::Expr. We can't use the `Item::sql_ast::Expr` code above
        // since we don't want to intersperse with spaces.
        ExprKind::SString(s_string_items) => {
            let string = translate_sstring(s_string_items, ctx)?;

            sql_ast::Expr::Identifier(sql_ast::Ident::new(string))
        }
        ExprKind::FString(f_string_items) => {
            let args = f_string_items
                .into_iter()
                .map(|item| match item {
                    InterpolateItem::String(string) => {
                        Ok(sql_ast::Expr::Value(Value::SingleQuotedString(string)))
                    }
                    InterpolateItem::Expr(node) => translate_expr_kind(node.kind, ctx),
                })
                .map(|r| r.map(|e| FunctionArg::Unnamed(FunctionArgExpr::Expr(e))))
                .collect::<Result<Vec<_>>>()?;

            sql_ast::Expr::Function(Function {
                name: ObjectName(vec![sql_ast::Ident::new("CONCAT")]),
                args,
                distinct: false,
                over: None,
                special: false,
            })
        }
        ExprKind::Literal(l) => match l {
            Literal::Null => sql_ast::Expr::Value(Value::Null),
            Literal::String(s) => sql_ast::Expr::Value(Value::SingleQuotedString(s)),
            Literal::Boolean(b) => sql_ast::Expr::Value(Value::Boolean(b)),
            Literal::Float(f) => sql_ast::Expr::Value(Value::Number(format!("{f:?}"), false)),
            Literal::Integer(i) => sql_ast::Expr::Value(Value::Number(format!("{i}"), false)),
            Literal::Date(value) => sql_ast::Expr::TypedString {
                data_type: sql_ast::DataType::Date,
                value,
            },
            Literal::Time(value) => sql_ast::Expr::TypedString {
                data_type: sql_ast::DataType::Time(None, sql_ast::TimezoneInfo::None),
                value,
            },
            Literal::Timestamp(value) => sql_ast::Expr::TypedString {
                data_type: sql_ast::DataType::Timestamp(None, sql_ast::TimezoneInfo::None),
                value,
            },
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
                sql_ast::Expr::Interval {
                    value: Box::new(translate_expr_kind(
                        ExprKind::Literal(Literal::Integer(vau.n)),
                        ctx,
                    )?),
                    leading_field: Some(sql_parser_datetime),
                    leading_precision: None,
                    last_field: None,
                    fractional_seconds_precision: None,
                }
            }
        },
        ExprKind::Switch(mut cases) => {
            let default = cases
                .last()
                .filter(|last| {
                    matches!(
                        last.condition.kind,
                        ExprKind::Literal(Literal::Boolean(true))
                    )
                })
                .map(|def| translate_expr_kind(def.value.kind.clone(), ctx))
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
                    let cond = translate_expr_kind(case.condition.kind, ctx)?;
                    let value = translate_expr_kind(case.value.kind, ctx)?;
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
        ExprKind::BuiltInFunction { name, args } => {
            super::std::translate_built_in(name, args, ctx)?
        }
    })
}

fn translate_cid(cid: CId, ctx: &mut Context) -> Result<sql_ast::Expr> {
    if ctx.pre_projection {
        log::debug!("translating {cid:?} pre projection");
        let decl = ctx.anchor.column_decls.get(&cid).expect("bad RQ ids");

        Ok(match decl {
            ColumnDecl::Compute(compute) => {
                let window = compute.window.clone();

                let expr = translate_expr_kind(compute.expr.kind.clone(), ctx)?;

                if let Some(window) = window {
                    translate_windowed(expr, window, ctx)?
                } else {
                    expr
                }
            }
            ColumnDecl::RelationColumn(tiid, _, col) => {
                let column = match col.clone() {
                    RelationColumn::Wildcard => "*".to_string(),
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
            ColumnDecl::RelationColumn(_, _, RelationColumn::Wildcard) => "*".to_string(),

            _ => {
                let name = ctx.anchor.column_names.get(&cid).cloned();
                name.expect("a name of this column to be set before generating SQL")
            }
        };

        let ident = translate_ident(table_name, Some(column), ctx);

        log::debug!("translating {cid:?} post projection: {ident:?}");

        let ident = sql_ast::Expr::CompoundIdentifier(ident);
        Ok(ident)
    }
}

pub(super) fn table_factor_of_tid(table_ref: TableRef, ctx: &Context) -> TableFactor {
    let decl = ctx.anchor.table_decls.get(&table_ref.source).unwrap();

    let relation_name = decl.name.clone().unwrap();
    TableFactor::Table {
        name: sql_ast::ObjectName(translate_ident(Some(relation_name), None, ctx)),
        alias: if decl.name == table_ref.name {
            None
        } else {
            table_ref.name.map(|ident| TableAlias {
                name: translate_ident_part(ident, ctx),
                columns: vec![],
            })
        },
        args: None,
        with_hints: vec![],
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
            InterpolateItem::Expr(node) => {
                translate_expr_kind(node.kind, ctx).map(|expr| expr.to_string())
            }
        })
        .collect::<Result<Vec<String>>>()?
        .join(""))
}

pub(super) fn translate_query_sstring(
    items: Vec<crate::ast::pl::InterpolateItem<Expr>>,
    context: &mut Context,
) -> Result<sql_ast::Query> {
    let string = translate_sstring(items, context)?;

    let prefix = if let Some(string) = string.trim().get(0..7) {
        string
    } else {
        ""
    };

    if prefix.eq_ignore_ascii_case("SELECT ") {
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

    bail!(Error::new(Reason::Simple(
        "s-strings representing a table must start with `SELECT `".to_string()
    ))
    .with_help("this is a limitation by current compiler implementation"))
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
    Top {
        quantity: Some(
            translate_expr_kind(ExprKind::Literal(Literal::Integer(take)), ctx).unwrap(),
        ),
        with_ties: false,
        percent: false,
    }
}
pub(super) fn try_into_exprs(cids: Vec<CId>, ctx: &mut Context) -> Result<Vec<sql_ast::Expr>> {
    cids.into_iter()
        .map(|cid| translate_cid(cid, ctx))
        .try_collect()
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

        return Ok(SelectItem::ExprWithAlias {
            alias: translate_ident_part(ident, ctx),
            expr,
        });
    }

    Ok(SelectItem::UnnamedExpr(expr))
}

fn try_into_is_null(
    op: &BinOp,
    a: &Expr,
    b: &Expr,
    ctx: &mut Context,
) -> Result<Option<sql_ast::Expr>> {
    if matches!(op, BinOp::Eq) || matches!(op, BinOp::Ne) {
        let expr = if matches!(a.kind, ExprKind::Literal(Literal::Null)) {
            b.kind.clone()
        } else if matches!(b.kind, ExprKind::Literal(Literal::Null)) {
            a.kind.clone()
        } else {
            return Ok(None);
        };

        let strength =
            sql_ast::Expr::IsNull(Box::new(sql_ast::Expr::Value(Value::Null))).binding_strength();
        let expr = translate_operand(expr, strength, false, ctx)?;

        return Ok(Some(if matches!(op, BinOp::Eq) {
            sql_ast::Expr::IsNull(expr)
        } else {
            sql_ast::Expr::IsNotNull(expr)
        }));
    }

    Ok(None)
}

fn try_into_between(
    op: &BinOp,
    a: &Expr,
    b: &Expr,
    ctx: &mut Context,
) -> Result<Option<sql_ast::Expr>> {
    if !matches!(op, BinOp::And) {
        return Ok(None);
    }
    let Some((a, b)) = a.kind.as_binary().zip(b.kind.as_binary()) else {
        return Ok(None);
    };
    if !(matches!(a.1, BinOp::Gte) && matches!(b.1, BinOp::Lte)) {
        return Ok(None);
    }
    if a.0 != b.0 {
        return Ok(None);
    }

    Ok(Some(sql_ast::Expr::Between {
        expr: translate_operand(a.0.kind.clone(), 0, false, ctx)?,
        negated: false,
        low: translate_operand(a.2.kind.clone(), 0, false, ctx)?,
        high: translate_operand(b.2.kind.clone(), 0, false, ctx)?,
    }))
}

fn translate_windowed(
    expr: sql_ast::Expr,
    window: Window,
    ctx: &mut Context,
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
        partition_by: try_into_exprs(window.partition, ctx)?,
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

pub(super) fn translate_join(
    (side, with, filter): (JoinSide, TableRef, Expr),
    ctx: &mut Context,
) -> Result<Join> {
    let constraint = JoinConstraint::On(translate_expr_kind(filter.kind, ctx)?);

    Ok(Join {
        relation: table_factor_of_tid(with, ctx),
        join_operator: match side {
            JoinSide::Inner => JoinOperator::Inner(constraint),
            JoinSide::Left => JoinOperator::LeftOuter(constraint),
            JoinSide::Right => JoinOperator::RightOuter(constraint),
            JoinSide::Full => JoinOperator::FullOuter(constraint),
        },
    })
}

/// Translate a PRQL Ident to a Vec of SQL Idents.
// We return a vec of SQL Idents because sqlparser sometimes uses
// [ObjectName](sql_ast::ObjectName) and sometimes uses
// [sql_ast::Expr::CompoundIdentifier](sql_ast::Expr::CompoundIdentifier), each of which
// contains `Vec<Ident>`.
pub(super) fn translate_ident(
    relation_name: Option<String>,
    column: Option<String>,
    ctx: &Context,
) -> Vec<sql_ast::Ident> {
    let mut parts = Vec::with_capacity(4);
    if !ctx.omit_ident_prefix || column.is_none() {
        if let Some(relation) = relation_name {
            // Special-case this for BigQuery, Ref #852
            if matches!(ctx.dialect.dialect(), Dialect::BigQuery) {
                parts.push(relation);
            } else {
                parts.extend(relation.split('.').map(|s| s.to_string()));
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
    // We'll remove this when we get the new dbt plugin working (so no need to
    // integrate into the regex)
    let is_jinja = ident.starts_with("{{") && ident.ends_with("}}");
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

    if is_jinja || is_bare && !is_keyword(&ident) {
        sql_ast::Ident::new(ident)
    } else {
        sql_ast::Ident::with_quote(ctx.dialect.ident_quote(), ident)
    }
}

/// Wraps into parenthesis if binding strength would be less than min_strength
fn translate_operand(
    expr: ExprKind,
    parent_strength: i32,
    fix_associativity: bool,
    context: &mut Context,
) -> Result<Box<sql_ast::Expr>> {
    let expr = Box::new(translate_expr_kind(expr, context)?);

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
}
