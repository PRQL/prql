//! Contains functions that compile [crate::ir] nodes into [sqlparser] nodes.

use anyhow::{bail, Result};
use itertools::Itertools;
use sqlparser::ast::{
    self as sql_ast, BinaryOperator, DateTimeField, Function, FunctionArg, FunctionArgExpr, Join,
    JoinConstraint, JoinOperator, ObjectName, OrderByExpr, SelectItem, TableFactor, Top,
    UnaryOperator, Value, WindowFrameBound,
};

use crate::ast::{
    BinOp, ColumnSort, Dialect, InterpolateItem, JoinSide, Literal, Range, SortDirection,
    WindowKind,
};
use crate::error::{Error, Reason};
use crate::ir::*;
use crate::utils::OrMap;

use super::translator::Context;

pub(super) fn translate_expr_kind(item: ExprKind, context: &mut Context) -> Result<sql_ast::Expr> {
    Ok(match item {
        ExprKind::ColumnRef(cid) => {
            let (table, column) = context.anchor.materialize_name(&cid);

            sql_ast::Expr::CompoundIdentifier(translate_ident(table, Some(column), context))
        }
        ExprKind::Binary { op, left, right } => {
            if let Some(is_null) = try_into_is_null(&op, &left, &right, context)? {
                is_null
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
                    BinOp::Coalesce => unreachable!(),
                };

                let strength = op.binding_strength();
                let left = translate_operand(left.kind, strength, !op.associates_left(), context)?;
                let right =
                    translate_operand(right.kind, strength, !op.associates_right(), context)?;
                sql_ast::Expr::BinaryOp { left, right, op }
            }
        }

        ExprKind::Unary { op, expr } => {
            let op = match op {
                UnOp::Neg => UnaryOperator::Minus,
                UnOp::Not => UnaryOperator::Not,
            };
            let expr = translate_operand(expr.kind, op.binding_strength(), false, context)?;
            sql_ast::Expr::UnaryOp { op, expr }
        }

        ExprKind::Range(r) => {
            fn assert_bound(bound: Option<Box<Expr>>) -> Result<Expr, Error> {
                bound.map(|b| *b).ok_or_else(|| {
                    Error::new(Reason::Simple(
                        "range requires both bounds to be used this way".to_string(),
                    ))
                })
            }
            let start: sql_ast::Expr = translate_expr_kind(assert_bound(r.start)?.kind, context)?;
            let end: sql_ast::Expr = translate_expr_kind(assert_bound(r.end)?.kind, context)?;
            sql_ast::Expr::Identifier(sql_ast::Ident::new(format!("{} AND {}", start, end)))
        }
        // Fairly hacky — convert everything to a string, then concat it,
        // then convert to sql_ast::Expr. We can't use the `Item::sql_ast::Expr` code above
        // since we don't want to intersperse with spaces.
        ExprKind::SString(s_string_items) => {
            let string = s_string_items
                .into_iter()
                .map(|s_string_item| match s_string_item {
                    InterpolateItem::String(string) => Ok(string),
                    InterpolateItem::Expr(node) => {
                        translate_expr_kind(node.kind, context).map(|expr| expr.to_string())
                    }
                })
                .collect::<Result<Vec<String>>>()?
                .join("");
            sql_ast::Expr::Identifier(sql_ast::Ident::new(string))
        }
        ExprKind::FString(f_string_items) => {
            let args = f_string_items
                .into_iter()
                .map(|item| match item {
                    InterpolateItem::String(string) => {
                        Ok(sql_ast::Expr::Value(Value::SingleQuotedString(string)))
                    }
                    InterpolateItem::Expr(node) => translate_expr_kind(node.kind, context),
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
        // ExprKind::Windowed(window) => {
        //     let expr = translate_expr_kind(window.expr.kind, dialect)?;

        //     let default_frame = if window.sort.is_empty() {
        //         (WindowKind::Rows, Range::unbounded())
        //     } else {
        //         (WindowKind::Range, Range::from_ints(None, Some(0)))
        //     };

        //     let window = WindowSpec {
        //         partition_by: try_into_exprs(window.group, dialect)?,
        //         order_by: (window.sort)
        //             .into_iter()
        //             .map(|s| translate_column_sort(s, dialect))
        //             .try_collect()?,
        //         window_frame: if window.window == default_frame {
        //             None
        //         } else {
        //             Some(try_into_window_frame(window.window)?)
        //         },
        //     };

        //     sql_ast::Expr::Identifier(sql_ast::Ident::new(format!("{expr} OVER ({window})")))
        // }
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
                data_type: sql_ast::DataType::Time(sql_ast::TimezoneInfo::None),
                value,
            },
            Literal::Timestamp(value) => sql_ast::Expr::TypedString {
                data_type: sql_ast::DataType::Timestamp(sql_ast::TimezoneInfo::None),
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
                        context,
                    )?),
                    leading_field: Some(sql_parser_datetime),
                    leading_precision: None,
                    last_field: None,
                    fractional_seconds_precision: None,
                }
            }
        },
    })
}

pub(super) fn table_factor_of_tid(tid: &TId, context: &Context) -> TableFactor {
    let def = context.anchor.table_defs.get(tid).unwrap();

    TableFactor::Table {
        name: sql_ast::ObjectName(translate_ident(Some(def.name.clone()), None, context)),
        alias: None,
        args: None,
        with_hints: vec![],
    }
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

    if current
        .start
        .zip(current.end)
        .map(|(s, e)| e < s)
        .unwrap_or(false)
    {
        bail!("Range end is before its start.");
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

pub(super) fn top_of_i64(take: i64, context: &mut Context) -> Top {
    Top {
        quantity: Some(
            translate_expr_kind(ExprKind::Literal(Literal::Integer(take)), context).unwrap(),
        ),
        with_ties: false,
        percent: false,
    }
}
pub(super) fn try_into_exprs(
    nodes: Vec<Expr>,
    context: &mut Context,
) -> Result<Vec<sql_ast::Expr>> {
    nodes
        .into_iter()
        .map(|x| x.kind)
        .map(|item| translate_expr_kind(item, context))
        .try_collect()
}

pub(super) fn translate_select_item(cid: CId, context: &mut Context) -> Result<SelectItem> {
    let expr = context.anchor.materialize_expr(&cid);
    let expr = translate_expr_kind(expr.kind, context)?;

    let inferred_name = match &expr {
        sql_ast::Expr::Identifier(name) => Some(&name.value),
        sql_ast::Expr::CompoundIdentifier(parts) => parts.last().map(|p| &p.value),
        _ => None,
    };

    if let Some(alias) = context.anchor.get_column_name(&cid) {
        if Some(&alias) != inferred_name {
            return Ok(SelectItem::ExprWithAlias {
                alias: translate_ident_part(alias, context),
                expr,
            });
        }
    }

    Ok(SelectItem::UnnamedExpr(expr))
}

fn try_into_is_null(
    op: &BinOp,
    a: &Expr,
    b: &Expr,
    context: &mut Context,
) -> Result<Option<sql_ast::Expr>> {
    if matches!(op, BinOp::Eq) || matches!(op, BinOp::Ne) {
        let expr = if matches!(a.kind, ExprKind::Literal(Literal::Null)) {
            b.kind.clone()
        } else if matches!(b.kind, ExprKind::Literal(Literal::Null)) {
            a.kind.clone()
        } else {
            return Ok(None);
        };

        let min_strength =
            sql_ast::Expr::IsNull(Box::new(sql_ast::Expr::Value(Value::Null))).binding_strength();
        let expr = translate_operand(expr, min_strength, false, context)?;

        return Ok(Some(if matches!(op, BinOp::Eq) {
            sql_ast::Expr::IsNull(expr)
        } else {
            sql_ast::Expr::IsNotNull(expr)
        }));
    }

    Ok(None)
}

#[allow(dead_code)]
fn try_into_window_frame((kind, range): (WindowKind, Range<Expr>)) -> Result<sql_ast::WindowFrame> {
    fn parse_bound(bound: Expr) -> Result<WindowFrameBound> {
        let as_int = bound.kind.into_literal()?.into_integer()?;
        Ok(match as_int {
            0 => WindowFrameBound::CurrentRow,
            1.. => WindowFrameBound::Following(Some(as_int as u64)),
            _ => WindowFrameBound::Preceding(Some((-as_int) as u64)),
        })
    }

    Ok(sql_ast::WindowFrame {
        units: match kind {
            WindowKind::Rows => sql_ast::WindowFrameUnits::Rows,
            WindowKind::Range => sql_ast::WindowFrameUnits::Range,
        },
        start_bound: if let Some(start) = range.start {
            parse_bound(start)?
        } else {
            WindowFrameBound::Preceding(None)
        },
        end_bound: Some(if let Some(end) = range.end {
            parse_bound(end)?
        } else {
            WindowFrameBound::Following(None)
        }),
    })
}

pub(super) fn translate_column_sort(
    sort: &ColumnSort<CId>,
    context: &mut Context,
) -> Result<OrderByExpr> {
    let (table, column) = context.anchor.materialize_name(&sort.column);
    Ok(OrderByExpr {
        expr: sql_ast::Expr::CompoundIdentifier(translate_ident(table, Some(column), context)),
        asc: if matches!(sort.direction, SortDirection::Asc) {
            None // default order is ASC, so there is no need to emit it
        } else {
            Some(false)
        },
        nulls_first: None,
    })
}

pub(super) fn filter_of_filters(
    conditions: Vec<Expr>,
    context: &mut Context,
) -> Result<Option<sql_ast::Expr>> {
    let mut condition = None;
    for filter in conditions {
        if let Some(left) = condition {
            condition = Some(Expr {
                kind: ExprKind::Binary {
                    op: BinOp::And,
                    left: Box::new(left),
                    right: Box::new(filter),
                },
                span: None,
            })
        } else {
            condition = Some(filter)
        }
    }

    condition
        .map(|n| translate_expr_kind(n.kind, context))
        .transpose()
}

pub(super) fn translate_join(t: &Transform, context: &mut Context) -> Result<Join> {
    if let Transform::Join { side, with, filter } = t {
        let constraint = JoinConstraint::On(translate_expr_kind(filter.kind.clone(), context)?);

        Ok(Join {
            relation: table_factor_of_tid(with, context),
            join_operator: match *side {
                JoinSide::Inner => JoinOperator::Inner(constraint),
                JoinSide::Left => JoinOperator::LeftOuter(constraint),
                JoinSide::Right => JoinOperator::RightOuter(constraint),
                JoinSide::Full => JoinOperator::FullOuter(constraint),
            },
        })
    } else {
        unreachable!()
    }
}

/// Translate a PRQL Ident to a Vec of SQL Idents.
// We return a vec of SQL Idents because sqlparser sometimes uses
// [ObjectName](sql_ast::ObjectName) and sometimes uses
// [sql_ast::Expr::CompoundIdentifier](sql_ast::Expr::CompoundIdentifier), each of which
// contains `Vec<Ident>`.
pub(super) fn translate_ident(
    relation_name: Option<String>,
    column: Option<String>,
    context: &Context,
) -> Vec<sql_ast::Ident> {
    let mut parts = Vec::with_capacity(4);
    if !context.omit_ident_prefix || column.is_none() {
        if let Some(relation) = relation_name {
            // Special-case this for BigQuery, Ref #852
            if matches!(context.dialect.dialect(), Dialect::BigQuery) {
                parts.push(relation);
            } else {
                parts.extend(relation.split('.').map(|s| s.to_string()));
            }
        }
    }

    parts.extend(column);

    parts
        .into_iter()
        .map(|x| translate_ident_part(x, context))
        .collect()
}

pub(super) fn translate_ident_part(ident: String, context: &Context) -> sql_ast::Ident {
    let is_jinja = ident.starts_with("{{") && ident.ends_with("}}");

    // TODO: can probably represent these with a single regex
    fn starting_forbidden(c: char) -> bool {
        !(('a'..='z').contains(&c) || matches!(c, '_' | '$'))
    }
    fn subsequent_forbidden(c: char) -> bool {
        !(('a'..='z').contains(&c) || ('0'..='9').contains(&c) || matches!(c, '_' | '$'))
    }

    let is_asterisk = ident == "*";

    if !is_asterisk
        && !is_jinja
        && (ident.is_empty()
            || ident.starts_with(starting_forbidden)
            || (ident.chars().count() > 1 && ident.contains(subsequent_forbidden)))
    {
        sql_ast::Ident::with_quote(context.dialect.ident_quote(), ident)
    } else {
        sql_ast::Ident::new(ident)
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
    use crate::ast::Range;
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

        // We can't get 5..6 from 5..6.
        assert!(range_of_ranges(vec![range2.clone(), range2.clone()]).is_err());

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
