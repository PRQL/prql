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

            Transform::Filter(filter.coerce_to_vec())
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
                    let (column, direction) = match node.item {
                        Item::NamedExpr(named_expr) => {
                            let direction = match named_expr.name.as_str() {
                                "asc" => SortDirection::Asc,
                                "desc" => SortDirection::Desc,
                                _ => {
                                    return Err(Error::new(Reason::Expected {
                                        who: Some("sort".to_string()),
                                        expected: "asc or desc".to_string(),
                                        found: named_expr.name,
                                    })
                                    .with_span(node.span))
                                }
                            };
                            (*named_expr.expr, direction)
                        }
                        _ => (node, SortDirection::default()),
                    };

                    if matches!(column.item, Item::Ident(_)) {
                        Ok(ColumnSort { direction, column })
                    } else {
                        Err(Error::new(Reason::Simple(
                            "`sort` expects column name, not expression".to_string(),
                        ))
                        .with_help("you can introduce a new column with `derive`")
                        .with_span(column.span))
                    }
                })
                .try_collect()?;

            Transform::Sort(by)
        }
        "take" => {
            let ([expr], []) = unpack(func_call, [])?;

            Transform::Take(expr.discard_name()?.item.into_raw()?.parse()?)
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

            let pipeline = Box::new(pipeline);

            Transform::Group { by, pipeline }
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

    use crate::parse;
    use crate::semantic::resolve;
    use crate::sql::load_std_lib;

    #[test]
    fn test_simple_casts() {
        let query = parse(r#"filter upper country = "USA""#).unwrap();
        assert!(resolve(query.nodes, None).is_err());

        let query = parse(r#"take"#).unwrap();
        assert!(resolve(query.nodes, None).is_err());
    }

    #[test]
    fn test_aggregate_positional_arg() {
        let stdlib = load_std_lib().unwrap();
        let (_, context) = resolve(stdlib, None).unwrap();
        let context = Some(context);

        // distinct query #292
        let query = parse(
            "
        from c_invoice
        group invoice_no (
            take 1
        )
        ",
        )
        .unwrap();
        // TODO: this test
        assert!(resolve(query.nodes, context.clone()).is_err());
        /*
        let (result, _) = resolve(query.nodes, context.clone()).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: c_invoice
                    alias: ~
                    declared_at: 58
              - Transform:
                  Group:
                    by:
                      - Ident: invoice_no
                    pipeline:
                      Pipeline:
                        value: ~
                        functions:
                          - Transform:
                              Take: 1
        "###);
        */

        // oops, two arguments #339
        let query = parse(
            "
        from c_invoice
        aggregate average amount
        ",
        )
        .unwrap();
        let result = resolve(query.nodes, context.clone());
        assert!(result.is_err());

        // oops, two arguments
        let query = parse(
            "
        from c_invoice
        group date (aggregate average amount)
        ",
        )
        .unwrap();
        let result = resolve(query.nodes, context.clone());
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
        let (result, _) = resolve(query.nodes, context).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            value: ~
            functions:
              - Transform:
                  From:
                    name: c_invoice
                    alias: ~
                    declared_at: 57
              - Transform:
                  Group:
                    by:
                      - Ident: date
                    pipeline:
                      Pipeline:
                        value: ~
                        functions:
                          - Transform:
                              Aggregate:
                                assigns:
                                  - Ident: "<unnamed>"
                                by:
                                  - Ident: "<ref>"
        "###);
    }
}
