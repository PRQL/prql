use std::collections::HashMap;

use itertools::Itertools;
use serde::Deserialize;

use crate::ir::generic::{SortDirection, WindowKind};
use crate::ir::pl::*;

use crate::semantic::ast_expand::{restrict_null_literal, try_restrict_range};
use crate::semantic::write_pl;
use crate::{Error, Reason, Result, WithErrorInfo};

/// try to convert function call with enough args into transform
#[allow(clippy::boxed_local)]
pub fn resolve_special_func(expr: Expr) -> Result<Expr> {
    let ExprKind::RqOperator { name, args } = expr.kind else {
        unreachable!()
    };

    let (kind, input) = match name.as_str() {
        "select" => {
            let [assigns, tbl] = unpack::<2>(args);
            (TransformKind::Select { assigns }, tbl)
        }
        "filter" => {
            let [filter, tbl] = unpack::<2>(args);
            (TransformKind::Filter { filter }, tbl)
        }
        "derive" => {
            let [assigns, tbl] = unpack::<2>(args);
            (TransformKind::Derive { assigns }, tbl)
        }
        "aggregate" => {
            let [assigns, tbl] = unpack::<2>(args);
            (TransformKind::Aggregate { assigns }, tbl)
        }
        "sort" => {
            let [by, tbl] = unpack::<2>(args);

            let by_fields = by.try_cast(|x| x.into_tuple(), Some("sort"), "tuple")?;
            let by = by_fields
                .into_iter()
                .map(|expr| {
                    let (column, direction) = match expr.kind {
                        ExprKind::RqOperator { name, mut args } if name == "std.neg" => {
                            (args.remove(0), SortDirection::Desc)
                        }
                        _ => (expr, SortDirection::default()),
                    };
                    let column = Box::new(column);

                    ColumnSort { direction, column }
                })
                .collect();

            (TransformKind::Sort { by }, tbl)
        }
        "take" => {
            let [expr, tbl] = unpack::<2>(args);

            let range = if let ExprKind::Literal(Literal::Integer(n)) = expr.kind {
                range_from_ints(None, Some(n))
            } else {
                match try_restrict_range(*expr) {
                    Ok((start, end)) => Range {
                        start: restrict_null_literal(start).map(Box::new),
                        end: restrict_null_literal(end).map(Box::new),
                    },
                    Err(expr) => {
                        return Err(Error::new(Reason::Expected {
                            who: Some("`take`".to_string()),
                            expected: "int or range".to_string(),
                            found: write_pl(expr.clone()),
                        })
                        // Possibly this should refer to the item after the `take` where
                        // one exists?
                        .with_span(expr.span));
                    }
                }
            };

            (TransformKind::Take { range }, tbl)
        }
        "join" => {
            let [with, filter, tbl] = unpack::<3>(args);

            let side = {
                JoinSide::Inner
                // let span = side.span;
                // let ident = side.try_cast(ExprKind::into_ident, Some("side"), "ident")?;
                // match ident.name.as_str() {
                //     "inner" => JoinSide::Inner,
                //     "left" => JoinSide::Left,
                //     "right" => JoinSide::Right,
                //     "full" => JoinSide::Full,

                //     found => {
                //         return Err(Error::new(Reason::Expected {
                //             who: Some("`side`".to_string()),
                //             expected: "inner, left, right or full".to_string(),
                //             found: found.to_string(),
                //         })
                //         .with_span(span))
                //     }
                // }
            };

            (TransformKind::Join { side, with, filter }, tbl)
        }
        "group" => {
            let [by, pipeline, tbl] = unpack::<3>(args);
            (TransformKind::Group { by, pipeline }, tbl)
        }
        "window" => {
            let [rows, range, expanding, rolling, pipeline, tbl] = unpack::<6>(args);

            let expanding = {
                let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: write_pl(*expanding.clone()),
                    })
                    .with_span(expanding.span)
                })?
            };

            let rolling = {
                let as_int = rolling.kind.as_literal().and_then(|x| x.as_integer());

                *as_int.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `rolling`".to_string()),
                        expected: "a number".to_string(),
                        found: write_pl(*rolling.clone()),
                    })
                    .with_span(rolling.span)
                })?
            };

            let rows = into_literal_range(try_restrict_range(*rows).unwrap())?;

            let range = into_literal_range(try_restrict_range(*range).unwrap())?;

            let (kind, start, end) = if expanding {
                (WindowKind::Rows, None, Some(0))
            } else if rolling > 0 {
                (WindowKind::Rows, Some(-rolling + 1), Some(0))
            } else if !range_is_empty(&rows) {
                (WindowKind::Rows, rows.0, rows.1)
            } else if !range_is_empty(&range) {
                (WindowKind::Range, range.0, range.1)
            } else {
                (WindowKind::Rows, None, None)
            };
            // let start = Expr::new(start.map_or(Literal::Null, Literal::Integer));
            // let end = Expr::new(end.map_or(Literal::Null, Literal::Integer));
            let range = Range {
                start: start.map(Literal::Integer).map(Expr::new).map(Box::new),
                end: end.map(Literal::Integer).map(Expr::new).map(Box::new),
            };

            let transform_kind = TransformKind::Window {
                kind,
                range,
                pipeline,
            };
            (transform_kind, tbl)
        }
        "append" => {
            let [bottom, top] = unpack::<2>(args);

            (TransformKind::Append(bottom), top)
        }
        "loop" => {
            let [pipeline, tbl] = unpack::<2>(args);
            (TransformKind::Loop(pipeline), tbl)
        }

        "in" => {
            let [pattern, value] = unpack::<2>(args);

            if pattern.ty.as_ref().map_or(false, |x| x.kind.is_array()) {
                return Ok(Expr {
                    kind: ExprKind::RqOperator {
                        name: "std.array_in".to_string(),
                        args: vec![*value, *pattern],
                    },
                    ..expr
                });
            }

            let pattern = match try_restrict_range(*pattern) {
                Ok((start, end)) => {
                    let start = restrict_null_literal(start);
                    let end = restrict_null_literal(end);

                    let start = start.map(|s| new_binop(*value.clone(), "std.gte", s));
                    let end = end.map(|e| new_binop(*value, "std.lte", e));

                    let res = maybe_binop(start, "std.and", end);
                    let res =
                        res.unwrap_or_else(|| Expr::new(ExprKind::Literal(Literal::Boolean(true))));
                    return Ok(res);
                }
                Err(expr) => expr,
            };
            let pattern = Expr {
                kind: pattern.kind,
                ..expr
            };

            return Err(Error::new(Reason::Expected {
                who: Some("std.in".to_string()),
                expected: "a pattern".to_string(),
                found: write_pl(pattern.clone()),
            })
            .with_span(pattern.span));
        }

        "tuple_every" => {
            let [list] = unpack::<1>(args);
            let list = list.kind.into_tuple().unwrap();

            let mut res = None;
            for item in list {
                res = maybe_binop(res, "std.and", Some(item));
            }
            let res = res.unwrap_or_else(|| Expr::new(ExprKind::Literal(Literal::Boolean(true))));

            return Ok(res);
        }

        "tuple_map" => {
            let [func, list] = unpack::<2>(args);
            let list_items = list.kind.into_tuple().unwrap();

            let list_items = list_items
                .into_iter()
                .map(|item| {
                    Expr::new(ExprKind::FuncCall(FuncCall::new_simple(
                        *func.clone(),
                        vec![item],
                    )))
                })
                .collect_vec();

            return Ok(Expr {
                kind: ExprKind::Tuple(list_items),
                ..*list
            });
        }

        "tuple_zip" => {
            let [a, b] = unpack::<2>(args);
            let a = a.kind.into_tuple().unwrap();
            let b = b.kind.into_tuple().unwrap();

            let mut res = Vec::new();
            for (a, b) in std::iter::zip(a, b) {
                res.push(Expr::new(ExprKind::Tuple(vec![a, b])));
            }

            return Ok(Expr::new(ExprKind::Tuple(res)));
        }

        "_eq" => {
            let [list] = unpack::<1>(args);
            let list = list.kind.into_tuple().unwrap();
            let [a, b]: [Expr; 2] = list.try_into().unwrap();

            let res = maybe_binop(Some(a), "std.eq", Some(b)).unwrap();
            return Ok(res);
        }

        "from_text" => {
            let [format, text_expr] = unpack::<2>(args);

            let text = match text_expr.kind {
                ExprKind::Literal(Literal::String(text)) => text,
                _ => {
                    return Err(Error::new(Reason::Expected {
                        who: Some("std.from_text".to_string()),
                        expected: "a string literal".to_string(),
                        found: format!("`{}`", write_pl(*text_expr.clone())),
                    })
                    .with_span(text_expr.span));
                }
            };

            let res = {
                let span = format.span;
                let format = format
                    .try_cast(ExprKind::into_ident, Some("format"), "ident")?
                    .name;
                match format.as_str() {
                    "csv" => from_text::parse_csv(&text)
                        .map_err(|r| Error::new_simple(r).with_span(span))?,
                    "json" => from_text::parse_json(&text)
                        .map_err(|r| Error::new_simple(r).with_span(span))?,

                    _ => {
                        return Err(Error::new(Reason::Expected {
                            who: Some("`format`".to_string()),
                            expected: "csv or json".to_string(),
                            found: format,
                        })
                        .with_span(span))
                    }
                }
            };

            // let ty = self.declare_table_for_literal(expr_id, Some(columns));

            let res = Expr::new(ExprKind::Array(
                res.rows
                    .into_iter()
                    .map(|row| {
                        Expr::new(ExprKind::Tuple(
                            row.into_iter()
                                .map(|lit| Expr::new(ExprKind::Literal(lit)))
                                .collect(),
                        ))
                    })
                    .collect(),
            ));
            let res = Expr {
                ty: None,
                id: text_expr.id,
                ..res
            };
            return Ok(res);
        }

        "prql_version" => {
            let ver = crate::compiler_version().to_string();
            return Ok(Expr {
                kind: ExprKind::Literal(Literal::String(ver)),
                ..expr
            });
        }

        "count" | "row_number" => {
            // HACK: these functions get `this`, resolved to `{x = {_self}}`, which
            // throws an error during lowering.
            // But because these functions don't *really* need an arg, we can just pass
            // a null instead.
            return Ok(Expr {
                needs_window: expr.needs_window,
                ..Expr::new(ExprKind::RqOperator {
                    name: format!("std.{name}"),
                    args: vec![Expr::new(Literal::Null)],
                })
            });
        }

        _ => return Err(Error::new_assert(format!("unknown operator {name}")).with_span(expr.span)),
    };

    let transform_call = TransformCall {
        kind: Box::new(kind),
        input,
        partition: None,
        frame: WindowFrame::default(),
        sort: Vec::new(),
    };
    Ok(Expr {
        kind: ExprKind::TransformCall(transform_call),
        ..expr
    })
}

fn range_is_empty(range: &(Option<i64>, Option<i64>)) -> bool {
    match (&range.0, &range.1) {
        (Some(s), Some(e)) => s > e,
        _ => false,
    }
}

fn range_from_ints(start: Option<i64>, end: Option<i64>) -> Range {
    let start = start.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    let end = end.map(|x| Box::new(Expr::new(ExprKind::Literal(Literal::Integer(x)))));
    Range { start, end }
}

fn into_literal_range(range: (Expr, Expr)) -> Result<(Option<i64>, Option<i64>)> {
    fn into_int(bound: Expr) -> Result<Option<i64>> {
        match bound.kind {
            ExprKind::Literal(Literal::Null) => Ok(None),
            ExprKind::Literal(Literal::Integer(i)) => Ok(Some(i)),
            _ => Err(Error::new_simple("expected an int literal").with_span(bound.span)),
        }
    }
    Ok((into_int(range.0)?, into_int(range.1)?))
}

/// Expects closure's args to be resolved.
/// Note that named args are before positional args, in order of declaration.
fn unpack<const P: usize>(func_args: Vec<Expr>) -> [Box<Expr>; P] {
    let boxed = func_args.into_iter().map(Box::new).collect_vec();
    boxed.try_into().expect("bad special function cast")
}

fn maybe_binop(left: Option<Expr>, op_name: &str, right: Option<Expr>) -> Option<Expr> {
    match (left, right) {
        (Some(left), Some(right)) => Some(new_binop(left, op_name, right)),
        (left, right) => left.or(right),
    }
}

fn new_binop(left: Expr, op_name: &str, right: Expr) -> Expr {
    Expr::new(ExprKind::RqOperator {
        name: op_name.to_string(),
        args: vec![left, right],
    })
}

mod from_text {
    use crate::ir::rq::RelationLiteral;

    use super::*;

    // TODO: Can we dynamically get the types, like in pandas? We need to put
    // quotes around strings and not around numbers.
    // https://stackoverflow.com/questions/64369887/how-do-i-read-csv-data-without-knowing-the-structure-at-compile-time
    pub fn parse_csv(text: &str) -> Result<RelationLiteral, String> {
        let text = text.trim();
        let mut rdr = csv::Reader::from_reader(text.as_bytes());

        fn parse_header(row: &csv::StringRecord) -> Vec<String> {
            row.into_iter().map(|x| x.to_string()).collect()
        }

        fn parse_row(row: csv::StringRecord) -> Vec<Literal> {
            row.into_iter()
                .map(|x| Literal::String(x.to_string()))
                .collect()
        }

        Ok(RelationLiteral {
            columns: parse_header(rdr.headers().map_err(|e| e.to_string())?),
            rows: rdr
                .records()
                .map(|row_result| row_result.map(parse_row))
                .try_collect()
                .map_err(|e| e.to_string())?,
        })
    }

    type JsonFormat1Row = HashMap<String, serde_json::Value>;

    #[derive(Deserialize)]
    struct JsonFormat2 {
        columns: Vec<String>,
        data: Vec<Vec<serde_json::Value>>,
    }

    fn map_json_primitive(primitive: serde_json::Value) -> Literal {
        use serde_json::Value::*;
        match primitive {
            Null => Literal::Null,
            Bool(bool) => Literal::Boolean(bool),
            Number(number) if number.is_i64() => Literal::Integer(number.as_i64().unwrap()),
            Number(number) if number.is_f64() => Literal::Float(number.as_f64().unwrap()),
            Number(_) => Literal::Null,
            String(string) => Literal::String(string),
            Array(_) => Literal::Null,
            Object(_) => Literal::Null,
        }
    }

    fn object_to_vec(
        mut row_map: HashMap<String, serde_json::Value>,
        columns: &[String],
    ) -> Vec<Literal> {
        columns
            .iter()
            .map(|c| {
                row_map
                    .remove(c)
                    .map(map_json_primitive)
                    .unwrap_or(Literal::Null)
            })
            .collect_vec()
    }

    pub fn parse_json(text: &str) -> Result<RelationLiteral, String> {
        parse_json1(text).or_else(|err1| {
            parse_json2(text)
                .map_err(|err2| format!("While parsing rows: {err1}\nWhile parsing object: {err2}"))
        })
    }

    fn parse_json1(text: &str) -> Result<RelationLiteral, String> {
        let data: Vec<JsonFormat1Row> = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut columns = data
            .first()
            .ok_or("json: no rows")?
            .keys()
            .cloned()
            .collect_vec();

        // JSON object keys are not ordered, so have to apply some order to produce
        // deterministic results
        columns.sort();

        let rows = data
            .into_iter()
            .map(|row_map| object_to_vec(row_map, &columns))
            .collect_vec();
        Ok(RelationLiteral { columns, rows })
    }

    fn parse_json2(text: &str) -> Result<RelationLiteral, String> {
        let JsonFormat2 { columns, data } =
            serde_json::from_str(text).map_err(|x| x.to_string())?;

        Ok(RelationLiteral {
            columns,
            rows: data
                .into_iter()
                .map(|row| row.into_iter().map(map_json_primitive).collect_vec())
                .collect_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::semantic::test::parse_resolve_and_lower;

    #[test]
    fn test_aggregate_positional_arg() {
        // distinct query #292

        assert_yaml_snapshot!(parse_resolve_and_lower("
        from db.c_invoice
        select invoice_no
        group invoice_no (
            take 1
        )
        ").unwrap(), @r###"
        ---
        def:
          version: ~
          other: {}
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  - c_invoice
              columns:
                - Single: invoice_no
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Single: invoice_no
                      - 0
                    - - Wildcard
                      - 1
                  name: c_invoice
              - Select:
                  - 0
              - Take:
                  range:
                    start: ~
                    end:
                      kind:
                        Literal:
                          Integer: 1
                      span: ~
                  partition:
                    - 0
                  sort: []
              - Select:
                  - 0
          columns:
            - Single: invoice_no
        "###);

        // oops, two arguments #339
        let result = parse_resolve_and_lower(
            "
        from db.c_invoice
        aggregate average amount
        ",
        );
        assert!(result.is_err());

        // oops, two arguments
        let result = parse_resolve_and_lower(
            "
        from db.c_invoice
        group issued_at (aggregate average amount)
        ",
        );
        assert!(result.is_err());

        // correct function call
        let ctx = crate::semantic::test::parse_and_resolve(
            "
        from db.c_invoice
        group issued_at (
            aggregate (average amount)
        )
        ",
        )
        .unwrap();
        let (res, _) = ctx.find_main_rel(&[]).unwrap().clone();
        let expr = res.clone();
        let expr = crate::semantic::resolver::test::erase_ids(expr);
        assert_yaml_snapshot!(expr);
    }

    #[test]
    fn test_transform_sort() {
        assert_yaml_snapshot!(parse_resolve_and_lower("
        from db.invoices
        sort {issued_at, -amount, +num_of_articles}
        sort issued_at
        sort (-issued_at)
        sort {issued_at}
        sort {-issued_at}
        ").unwrap(), @r###"
        ---
        def:
          version: ~
          other: {}
        tables:
          - id: 0
            name: ~
            relation:
              kind:
                ExternRef:
                  - invoices
              columns:
                - Single: issued_at
                - Single: amount
                - Single: num_of_articles
                - Wildcard
        relation:
          kind:
            Pipeline:
              - From:
                  source: 0
                  columns:
                    - - Single: issued_at
                      - 0
                    - - Single: amount
                      - 1
                    - - Single: num_of_articles
                      - 2
                    - - Wildcard
                      - 3
                  name: invoices
              - Sort:
                  - direction: Asc
                    column: 0
                  - direction: Desc
                    column: 1
                  - direction: Asc
                    column: 2
              - Sort:
                  - direction: Asc
                    column: 0
              - Sort:
                  - direction: Desc
                    column: 0
              - Sort:
                  - direction: Asc
                    column: 0
              - Sort:
                  - direction: Desc
                    column: 0
              - Select:
                  - 0
                  - 1
                  - 2
                  - 3
          columns:
            - Single: issued_at
            - Single: amount
            - Single: num_of_articles
            - Wildcard
        "###);
    }
}
