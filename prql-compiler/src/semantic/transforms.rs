use anyhow::{anyhow, bail, Result};
use itertools::Itertools;

use crate::ast::*;
use crate::error::{Error, Reason, Span, WithErrorInfo};

pub fn cast_transform(func_call: FuncCall, span: Option<Span>) -> Result<Transform> {
    Ok(match func_call.name.as_str() {
        "from" => {
            let ([with], []) = unpack(func_call, [])?;

            let (name, expr) = with.into_name_and_expr();

            let table_ref = TableRef {
                name: expr.unwrap(Item::into_ident, "ident").with_help(
                    "`from` does not support inline expressions. You can only pass a table name.",
                )?,
                alias: name,
                declared_at: None,
            };

            Transform::From(table_ref)
        }
        "select" => {
            let ([assigns], []) = unpack(func_call, [])?;

            Transform::Select(assigns.coerce_to_vec())
        }
        "filter" => {
            let ([filter], []) = unpack(func_call, [])?;

            Transform::Filter(Box::new(filter))
        }
        "derive" => {
            let ([assigns], []) = unpack(func_call, [])?;

            Transform::Derive(assigns.coerce_to_vec())
        }
        "aggregate" => {
            let ([assigns], []) = unpack(func_call, [])?;

            Transform::Aggregate {
                assigns: assigns.coerce_to_vec(),
                by: vec![],
            }
        }
        "sort" => {
            let ([by], []) = unpack(func_call, [])?;

            let by = by
                .coerce_to_vec()
                .into_iter()
                .map(|node| {
                    let (column, direction) = match &node.item {
                        Item::Ident(_) => (node.clone(), SortDirection::default()),
                        Item::Unary { op, expr: a }
                            if matches!((op, &a.item), (UnOp::Neg, Item::Ident(_))) =>
                        {
                            (*a.clone(), SortDirection::Desc)
                        }
                        _ => {
                            return Err(Error::new(Reason::Expected {
                                who: Some("sort".to_string()),
                                expected: "column name, optionally prefixed with + or -"
                                    .to_string(),
                                found: node.item.to_string(),
                            })
                            .with_span(node.span));
                        }
                    };

                    if matches!(column.item, Item::Ident(_)) {
                        Ok(ColumnSort { direction, column })
                    } else {
                        Err(Error::new(Reason::Expected {
                            who: Some("sort".to_string()),
                            expected: "column name".to_string(),
                            found: format!("`{}`", column.item),
                        })
                        .with_help("you can introduce a new column with `derive`")
                        .with_span(column.span))
                    }
                })
                .try_collect()?;

            Transform::Sort(by)
        }
        "take" => {
            let ([expr], []) = unpack(func_call, [])?;

            let range = match expr.discard_name()?.item {
                Item::Literal(Literal::Integer(n)) => Range::from_ints(None, Some(n)),
                Item::Range(range) => range,
                _ => unimplemented!(),
            };
            Transform::Take {
                range,
                by: vec![],
                sort: vec![],
            }
        }
        "join" => {
            let ([with, filter], [side]) = unpack(func_call, ["side"])?;

            let side = if let Some(side) = side {
                let span = side.span;
                let ident = side.unwrap(Item::into_ident, "ident")?;
                match ident.as_str() {
                    "inner" => JoinSide::Inner,
                    "left" => JoinSide::Left,
                    "right" => JoinSide::Right,
                    "full" => JoinSide::Full,

                    found => bail!(Error::new(Reason::Expected {
                        who: Some("side".to_string()),
                        expected: "inner, left, right or full".to_string(),
                        found: found.to_string()
                    })
                    .with_span(span)),
                }
            } else {
                JoinSide::Inner
            };

            let (with_alias, with) = with.into_name_and_expr();
            let with = TableRef {
                name: with.unwrap(Item::into_ident, "ident").with_help(
                    "`join` does not support inline expressions. You can only pass a table name.",
                )?,
                alias: with_alias,
                declared_at: None,
            };

            let filter = filter.discard_name()?.coerce_to_vec();
            let use_using = (filter.iter().map(|x| &x.item)).all(|x| matches!(x, Item::Ident(_)));

            let filter = if use_using {
                JoinFilter::Using(filter)
            } else {
                JoinFilter::On(filter)
            };

            Transform::Join { side, with, filter }
        }
        "group" => {
            let ([by, pipeline], []) = unpack(func_call, [])?;

            let by = by
                .coerce_to_vec()
                .into_iter()
                // check that they are only idents
                .map(|n| match n.item {
                    Item::Ident(_) => Ok(n),
                    _ => Err(Error::new(Reason::Simple(
                        "`group` expects only idents for the `by` argument".to_string(),
                    ))
                    .with_span(n.span)),
                })
                .try_collect()?;

            let pipeline = Box::new(Item::Pipeline(pipeline.coerce_to_pipeline()).into());

            Transform::Group { by, pipeline }
        }
        "window" => {
            let ([pipeline], [rows, range, expanding, rolling]) =
                unpack(func_call, ["rows", "range", "expanding", "rolling"])?;

            let expanding = if let Some(expanding) = expanding {
                let as_bool = expanding.item.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: format!("{}", expanding.item),
                    })
                    .with_span(expanding.span)
                })?
            } else {
                false
            };

            let rolling = if let Some(rolling) = rolling {
                let as_int = rolling.item.as_literal().and_then(|x| x.as_integer());

                *as_int.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `rolling`".to_string()),
                        expected: "a number".to_string(),
                        found: format!("{}", rolling.item),
                    })
                    .with_span(rolling.span)
                })?
            } else {
                0
            };

            let rows = if let Some(rows) = rows {
                Some(rows.item.into_range().map_err(|x| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `rows`".to_string()),
                        expected: "a range".to_string(),
                        found: format!("{}", x),
                    })
                    .with_span(rows.span)
                })?)
            } else {
                None
            };

            let range = if let Some(range) = range {
                Some(range.item.into_range().map_err(|x| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `range`".to_string()),
                        expected: "a range".to_string(),
                        found: format!("{}", x),
                    })
                    .with_span(range.span)
                })?)
            } else {
                None
            };

            let (kind, range) = if expanding {
                (WindowKind::Rows, Range::from_ints(None, Some(0)))
            } else if rolling > 0 {
                (
                    WindowKind::Rows,
                    Range::from_ints(Some(-rolling + 1), Some(0)),
                )
            } else if let Some(range) = rows {
                (WindowKind::Rows, range)
            } else if let Some(range) = range {
                (WindowKind::Range, range)
            } else {
                (WindowKind::Rows, Range::unbounded())
            };
            Transform::Window {
                range,
                kind,
                pipeline: Box::new(Item::Pipeline(pipeline.coerce_to_pipeline()).into()),
            }
        }
        unknown => bail!(Error::new(Reason::Expected {
            who: None,
            expected: "a known transform".to_string(),
            found: format!("`{unknown}`")
        })
        .with_span(span)
        .with_help("use one of: from, select, derive, filter, aggregate, group, join, sort, take")),
    })
}

fn unpack<const P: usize, const N: usize>(
    mut func_call: FuncCall,
    expected: [&str; N],
) -> Result<([Node; P], [Option<Node>; N])> {
    // named
    const NONE: Option<Node> = None;
    let mut named = [NONE; N];

    for (i, e) in expected.into_iter().enumerate() {
        if let Some(val) = func_call.named_args.remove(e) {
            named[i] = Some(*val);
        }
    }

    // positional
    let positional =
        (func_call.args.try_into()).map_err(|_| anyhow!("bad `{}` definition", func_call.name))?;

    Ok((positional, named))
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::semantic::load_std_lib;
    use crate::{parse, semantic::resolve};

    #[test]
    fn test_simple_casts() {
        let query = parse(r#"filter upper country = "USA""#).unwrap();
        assert!(resolve(query, None).is_err());

        let query = parse(r#"take"#).unwrap();
        assert!(resolve(query, None).is_err());
    }

    #[test]
    fn test_aggregate_positional_arg() {
        let context = Some(load_std_lib());

        // distinct query #292
        let query = parse(
            "
        from c_invoice
        select invoice_no
        group invoice_no (
            take 1
        )
        ",
        )
        .unwrap();
        let (result, _) = resolve(query, context.clone()).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            nodes:
              - Transform:
                  From:
                    name: c_invoice
                    alias: ~
                    declared_at: 79
              - Transform:
                  Select:
                    - Ident: invoice_no
              - Transform:
                  Group:
                    by:
                      - Ident: invoice_no
                    pipeline:
                      Pipeline:
                        nodes:
                          - Transform:
                              Take:
                                range:
                                  start: ~
                                  end:
                                    Literal:
                                      Integer: 1
                                by:
                                  - Ident: "<ref>"
                                sort: []
        "###);

        // oops, two arguments #339
        let query = parse(
            "
        from c_invoice
        aggregate average amount
        ",
        )
        .unwrap();
        let result = resolve(query, context.clone());
        assert!(result.is_err());

        // oops, two arguments
        let query = parse(
            "
        from c_invoice
        group date (aggregate average amount)
        ",
        )
        .unwrap();
        let result = resolve(query, context.clone());
        assert!(result.is_err());

        // correct function call
        let query = parse(
            "
        from c_invoice
        group date (
            aggregate (average amount)
        )
        ",
        )
        .unwrap();
        let (result, _) = resolve(query, context).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            nodes:
              - Transform:
                  From:
                    name: c_invoice
                    alias: ~
                    declared_at: 79
              - Transform:
                  Group:
                    by:
                      - Ident: date
                    pipeline:
                      Pipeline:
                        nodes:
                          - Transform:
                              Aggregate:
                                assigns:
                                  - Ident: "<unnamed>"
                                by:
                                  - Ident: "<ref>"
        "###);
    }

    #[test]
    fn test_transform_sort() {
        let context = Some(load_std_lib());

        let query = parse(
            "
        from invoices
        sort [issued_at, -amount, +num_of_articles]
        sort issued_at
        sort (-issued_at)
        sort [issued_at]
        sort [-issued_at]
        ",
        )
        .unwrap();

        let (result, _) = resolve(query, context).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            nodes:
              - Transform:
                  From:
                    name: invoices
                    alias: ~
                    declared_at: 79
              - Transform:
                  Sort:
                    - direction: Asc
                      column:
                        Ident: issued_at
                    - direction: Desc
                      column:
                        Ident: amount
                    - direction: Asc
                      column:
                        Ident: num_of_articles
              - Transform:
                  Sort:
                    - direction: Asc
                      column:
                        Ident: issued_at
              - Transform:
                  Sort:
                    - direction: Desc
                      column:
                        Ident: issued_at
              - Transform:
                  Sort:
                    - direction: Asc
                      column:
                        Ident: issued_at
              - Transform:
                  Sort:
                    - direction: Desc
                      column:
                        Ident: issued_at
        "###);
    }
}
