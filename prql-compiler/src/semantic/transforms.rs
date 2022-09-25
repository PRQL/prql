use anyhow::{anyhow, bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::AstFold;
use crate::ast::*;
use crate::error::{Error, Reason};

use super::resolver::Resolver;
use super::{Frame, FrameColumn};

/*
fn fold_node(&mut self, mut node: Node) -> Result<Node> {
    match node.item {
        Item::FuncCall(func_call) => {
            if let Some(transform) = cast_transform(&func_call, node.declared_at)? {
                node.item = Item::Transform(self.fold_transform(transform)?);
            } else {
                let func_def = self.context.declarations.get_func(node.declared_at)?;
                let return_type = func_def.return_ty.clone();

                let func_call = Item::FuncCall(self.fold_func_call(func_call)?);

                // wrap into windowed
                if Some(Ty::column()) <= return_type && !self.within_aggregate {
                    node.item = self.wrap_into_windowed(func_call, node.declared_at);
                    node.declared_at = None;
                } else {
                    node.item = func_call;
                }
            }
        }

        item => {
            node.item = fold_item(self, item)?;
        }
    }
    Ok(node)
} */

impl TransformKind {
    pub fn apply_to(&self, mut frame: Frame) -> Result<Frame> {
        Ok(match self {
            TransformKind::From(t) => match &t.ty {
                Some(Ty::Table(f)) => f.clone(),
                Some(ty) => bail!(
                    "`from` expected a table name, got `{}` of type `{ty}`",
                    t.name
                ),
                a => bail!("`from` expected a frame got `{t:?}` of type {a:?}"),
            },

            TransformKind::Select(assigns) => {
                frame.columns.clear();
                frame.apply_assigns(assigns);
                frame
            }
            TransformKind::Derive(assigns) => {
                frame.apply_assigns(assigns);
                frame
            }
            TransformKind::Group { pipeline, .. } => {
                frame.sort.clear();

                for transform in pipeline {
                    frame = transform.kind.apply_to(frame)?;
                }
                frame
            }
            TransformKind::Window { pipeline, .. } => {
                frame.sort.clear();

                for transform in pipeline {
                    frame = transform.kind.apply_to(frame)?;
                }
                frame
            }
            TransformKind::Aggregate { assigns, by } => {
                let old_columns = frame.columns.clone();

                frame.columns.clear();

                for b in by {
                    let id = b.declared_at.unwrap();
                    let col = old_columns.iter().find(|c| c == &&id);
                    let name = col.and_then(|c| match c {
                        FrameColumn::Named(n, _) => Some(n.clone()),
                        _ => None,
                    });

                    frame.push_column(name, id);
                }

                frame.apply_assigns(assigns);
                frame
            }
            TransformKind::Join { with, filter, .. } => {
                let table_id = with
                    .declared_at
                    .ok_or_else(|| anyhow!("unresolved table {with:?}"))?;
                frame.tables.push(table_id);
                frame.columns.push(FrameColumn::All(table_id));

                match filter {
                    JoinFilter::On(_) => {}
                    JoinFilter::Using(nodes) => {
                        for node in nodes {
                            let name = node.kind.as_ident().unwrap().clone();
                            let id = node.declared_at.unwrap();
                            frame.push_column(Some(name), id);
                        }
                    }
                }
                frame
            }
            TransformKind::Sort(sort) => {
                frame.sort = extract_sorts(sort)?;
                frame
            }
            TransformKind::Filter(_) | TransformKind::Take { .. } | TransformKind::Unique => frame,
        })

        // if !self.within_group.is_empty() {
        //     self.apply_group(&mut t)?;
        // }
        // if self.within_window.is_some() {
        //     self.apply_window(&mut t)?;
        // }
    }
}

pub fn extract_sorts(sort: &[ColumnSort]) -> Result<Vec<ColumnSort<usize>>> {
    sort.iter()
        .map(|s| {
            Ok(ColumnSort {
                column: (s.column.declared_at)
                    .ok_or_else(|| anyhow!("Unresolved ident in sort?"))?,
                direction: s.direction.clone(),
            })
        })
        .try_collect()
}

/*
impl TransformConstructor {
    fn wrap_into_windowed(&self, expr: Item, declared_at: Option<usize>) -> Item {
        const REF: &str = "<ref>";

        let mut expr: Node = expr.into();
        expr.declared_at = declared_at;

        let frame = self
            .within_window
            .clone()
            .unwrap_or((WindowKind::Rows, Range::unbounded()));

        let mut window = Windowed::new(expr, frame);

        if !self.within_group.is_empty() {
            window.group = (self.within_group)
                .iter()
                .map(|id| Node::new_ident(REF, *id))
                .collect();
        }
        if !self.sorted.is_empty() {
            window.sort = (self.sorted)
                .iter()
                .map(|s| ColumnSort {
                    column: Node::new_ident(REF, s.column),
                    direction: s.direction.clone(),
                })
                .collect();
        }

        Item::Windowed(window)
    }

    fn apply_group(&mut self, t: &mut Transform) -> Result<()> {
        match t {
            Transform::Select(_)
            | Transform::Derive(_)
            | Transform::Sort(_)
            | Transform::Window { .. } => {
                // ok
            }
            Transform::Aggregate { by, .. } => {
                *by = (self.within_group)
                    .iter()
                    .map(|id| Node::new_ident("<ref>", *id))
                    .collect();
            }
            Transform::Take { by, sort, .. } => {
                *by = (self.within_group)
                    .iter()
                    .map(|id| Node::new_ident("<ref>", *id))
                    .collect();

                *sort = (self.sorted)
                    .iter()
                    .map(|s| ColumnSort {
                        column: Node::new_ident("<ref>", s.column),
                        direction: s.direction.clone(),
                    })
                    .collect();
            }
            _ => {
                // TODO: attach span to this error
                bail!(Error::new(Reason::Simple(format!(
                    "transform `{}` is not allowed within group context",
                    t.as_ref()
                ))))
            }
        }
        Ok(())
    }

    fn apply_window(&mut self, t: &mut Transform) -> Result<()> {
        if !matches!(t, Transform::Select(_) | Transform::Derive(_)) {
            // TODO: attach span to this error
            bail!(Error::new(Reason::Simple(format!(
                "transform `{}` is not allowed within window context",
                t.as_ref()
            ))))
        }
        Ok(())
    }
} */

type PipelineAndTransform = (Option<Expr>, TransformKind);

/// try to convert function call with enough args into transform
pub fn cast_transform(
    resolver: &mut Resolver,
    func_call: FuncCurry,
) -> Result<Result<PipelineAndTransform, FuncCurry>> {
    // TODO: transform functions are currently matched by static id, which
    // may change if stdlib.prql is changed. This is neither ergonomic nor readable.
    Ok(Ok(match func_call.def_id {
        0 => {
            let ([with], []) = unpack::<1, 0>(func_call)?;

            (None, TransformKind::From(unpack_table_ref(with)?))
        }
        1 => {
            let ([assigns, pipeline], []) = unpack::<2, 0>(func_call)?;

            let assigns = resolver.context.extract_decls(assigns.coerce_to_vec())?;

            (Some(pipeline), TransformKind::Select(assigns))
        }
        2 => {
            let ([filter, pipeline], []) = unpack::<2, 0>(func_call)?;

            (Some(pipeline), TransformKind::Filter(Box::new(filter)))
        }
        3 => {
            let ([assigns, pipeline], []) = unpack::<2, 0>(func_call)?;

            let assigns = resolver.context.extract_decls(assigns.coerce_to_vec())?;

            (Some(pipeline), TransformKind::Derive(assigns))
        }
        4 => {
            let ([assigns, pipeline], []) = unpack::<2, 0>(func_call)?;

            let assigns = resolver.context.extract_decls(assigns.coerce_to_vec())?;
            let by = vec![];

            (Some(pipeline), TransformKind::Aggregate { assigns, by })
        }
        5 => {
            let ([by, pipeline], []) = unpack::<2, 0>(func_call)?;

            let by = by
                .coerce_to_vec()
                .into_iter()
                .map(|node| {
                    let (column, direction) = match &node.kind {
                        ExprKind::Ident(_) => (node.clone(), SortDirection::default()),
                        ExprKind::Unary { op, expr: a }
                            if matches!((op, &a.kind), (UnOp::Neg, ExprKind::Ident(_))) =>
                        {
                            (*a.clone(), SortDirection::Desc)
                        }
                        _ => {
                            return Err(Error::new(Reason::Expected {
                                who: Some("`sort`".to_string()),
                                expected: "column name, optionally prefixed with + or -"
                                    .to_string(),
                                found: node.kind.to_string(),
                            })
                            .with_span(node.span));
                        }
                    };

                    if matches!(column.kind, ExprKind::Ident(_)) {
                        Ok(ColumnSort { direction, column })
                    } else {
                        Err(Error::new(Reason::Expected {
                            who: Some("`sort`".to_string()),
                            expected: "column name".to_string(),
                            found: format!("`{}`", column.kind),
                        })
                        .with_help("you can introduce a new column with `derive`")
                        .with_span(column.span))
                    }
                })
                .try_collect()?;

            (Some(pipeline), TransformKind::Sort(by))
        }
        6 => {
            let ([expr, pipeline], []) = unpack::<2, 0>(func_call)?;

            let range = match expr.kind {
                ExprKind::Literal(Literal::Integer(n)) => Range::from_ints(None, Some(n)),
                ExprKind::Range(range) => range,
                i => unimplemented!("`take` range: {i}"),
            };
            (
                Some(pipeline),
                TransformKind::Take {
                    range,
                    by: vec![],
                    sort: vec![],
                },
            )
        }
        7 => {
            let ([with, filter, pipeline], [side]) = unpack::<3, 1>(func_call)?;

            let side = if let Some(side) = side {
                let span = side.span;
                let ident = side.unwrap(ExprKind::into_ident, "ident")?;
                match ident.as_str() {
                    "inner" => JoinSide::Inner,
                    "left" => JoinSide::Left,
                    "right" => JoinSide::Right,
                    "full" => JoinSide::Full,

                    found => bail!(Error::new(Reason::Expected {
                        who: Some("`side`".to_string()),
                        expected: "inner, left, right or full".to_string(),
                        found: found.to_string()
                    })
                    .with_span(span)),
                }
            } else {
                JoinSide::Inner
            };

            let with = unpack_table_ref(with)?;

            let filter = filter.coerce_to_vec();
            let use_using =
                (filter.iter().map(|x| &x.kind)).all(|x| matches!(x, ExprKind::Ident(_)));

            let filter = if use_using {
                JoinFilter::Using(filter)
            } else {
                JoinFilter::On(filter)
            };

            (Some(pipeline), TransformKind::Join { side, with, filter })
        }
        8 => {
            let ([by, pipeline, pl], []) = unpack::<3, 0>(func_call)?;

            let by = by
                .coerce_to_vec()
                .into_iter()
                // check that they are only idents
                .map(|n| match n.kind {
                    ExprKind::Ident(_) => Ok(n),
                    _ => Err(Error::new(Reason::Simple(
                        "`group` expects only idents for the `by` argument".to_string(),
                    ))
                    .with_span(n.span)),
                })
                .try_collect()?;

            // simulate evaluation of the inner pipeline
            let mut value = Expr::from(ExprKind::ResolvedPipeline(vec![]));
            value.ty = pl.ty.clone();

            let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(pipeline),
                args: vec![value],
                named_args: Default::default(),
            }));
            let pipeline = resolver
                .fold_expr(pipeline)?
                .kind
                .into_resolved_pipeline()?;

            (Some(pl), TransformKind::Group { by, pipeline })
        }
        9 => {
            let ([pipeline, pl], [rows, range, expanding, rolling]) = unpack::<2, 4>(func_call)?;

            let expanding = if let Some(expanding) = expanding {
                let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: format!("{}", expanding.kind),
                    })
                    .with_span(expanding.span)
                })?
            } else {
                false
            };

            let rolling = if let Some(rolling) = rolling {
                let as_int = rolling.kind.as_literal().and_then(|x| x.as_integer());

                *as_int.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `rolling`".to_string()),
                        expected: "a number".to_string(),
                        found: format!("{}", rolling.kind),
                    })
                    .with_span(rolling.span)
                })?
            } else {
                0
            };

            let rows = if let Some(rows) = rows {
                Some(rows.kind.into_range().map_err(|x| {
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
                Some(range.kind.into_range().map_err(|x| {
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

            // simulate evaluation of the inner pipeline
            let mut value = Expr::from(ExprKind::ResolvedPipeline(vec![]));
            value.ty = pl.ty.clone();

            let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(pipeline),
                args: vec![value],
                named_args: Default::default(),
            }));
            let pipeline = resolver
                .fold_expr(pipeline)?
                .kind
                .into_resolved_pipeline()?;

            (
                Some(pl),
                TransformKind::Window {
                    range,
                    kind,
                    pipeline,
                },
            )
        }
        _ => return Ok(Err(func_call)),
    }))
}

fn unpack_table_ref(expr: Expr) -> Result<TableRef> {
    let alias = expr.alias;

    let declared_at = expr.declared_at;
    let ty = expr.ty.clone();

    let name = expr.kind.into_ident().map_err(|e| {
        Error::new(Reason::Expected {
            who: None,
            expected: "table name".to_string(),
            found: format!("`{e}`"),
        })
        .with_span(expr.span)
        .with_help(
            r"Inline table expressions are not yet supported.
            You can define new table with `table my_table = (...)`",
        )
    })?;
    Ok(TableRef {
        name,
        alias,
        declared_at,
        ty,
    })
}

fn unpack<const P: usize, const N: usize>(
    func_call: FuncCurry,
) -> Result<([Expr; P], [Option<Expr>; N])> {
    let named = func_call.named_args.try_into().expect("bad transform cast");
    let positional = (func_call.args.try_into()).expect("bad transform cast");

    Ok((positional, named))
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::{parse, semantic::resolve};

    #[test]
    fn test_aggregate_positional_arg() {
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
        let (result, _) = resolve(query, None).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables: []
        main_pipeline:
          - kind:
              From:
                name: c_invoice
                alias: ~
                declared_at: 29
                ty:
                  Table:
                    columns:
                      - All: 29
                    sort: []
                    tables: []
            is_complex: false
            ty:
              columns:
                - All: 29
              sort: []
              tables: []
            span:
              start: 9
              end: 23
          - kind:
              Select:
                - Ident: invoice_no
                  ty:
                    Parameterized:
                      - Assigns
                      - Type:
                          Literal: Column
            is_complex: false
            ty:
              columns:
                - Unnamed: 30
              sort: []
              tables: []
            span:
              start: 32
              end: 49
          - kind:
              Group:
                by:
                  - Ident: invoice_no
                    ty: Infer
                pipeline:
                  - kind:
                      Take:
                        range:
                          start: ~
                          end:
                            Literal:
                              Integer: 1
                        by: []
                        sort: []
                    is_complex: false
                    ty:
                      columns:
                        - Unnamed: 30
                      sort: []
                      tables: []
                    span: ~
            is_complex: false
            ty:
              columns:
                - Unnamed: 30
              sort: []
              tables: []
            span:
              start: 58
              end: 105
        "###);

        // oops, two arguments #339
        let query = parse(
            "
        from c_invoice
        aggregate average amount
        ",
        )
        .unwrap();
        let result = resolve(query, None);
        assert!(result.is_err());

        // oops, two arguments
        let query = parse(
            "
        from c_invoice
        group date (aggregate average amount)
        ",
        )
        .unwrap();
        let result = resolve(query, None);
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
        let (result, _) = resolve(query, None).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables: []
        main_pipeline:
          - kind:
              From:
                name: c_invoice
                alias: ~
                declared_at: 29
                ty:
                  Table:
                    columns:
                      - All: 29
                    sort: []
                    tables: []
            is_complex: false
            ty:
              columns:
                - All: 29
              sort: []
              tables: []
            span:
              start: 9
              end: 23
          - kind:
              Group:
                by:
                  - Ident: date
                    ty: Infer
                pipeline:
                  - kind:
                      Aggregate:
                        assigns:
                          - Ident: "<unnamed>"
                        by: []
                    is_complex: false
                    ty:
                      columns:
                        - Unnamed: 32
                      sort: []
                      tables: []
                    span: ~
            is_complex: false
            ty:
              columns:
                - Unnamed: 32
              sort: []
              tables: []
            span:
              start: 32
              end: 93
        "###);
    }

    #[test]
    fn test_transform_sort() {
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

        let (result, _) = resolve(query, None).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables: []
        main_pipeline:
          - kind:
              From:
                name: invoices
                alias: ~
                declared_at: 29
                ty:
                  Table:
                    columns:
                      - All: 29
                    sort: []
                    tables: []
            is_complex: false
            ty:
              columns:
                - All: 29
              sort: []
              tables: []
            span:
              start: 9
              end: 22
          - kind:
              Sort:
                - direction: Asc
                  column:
                    Ident: issued_at
                    ty: Infer
                - direction: Desc
                  column:
                    Ident: amount
                    ty: Infer
                - direction: Asc
                  column:
                    Ident: num_of_articles
                    ty: Infer
            is_complex: false
            ty:
              columns:
                - All: 29
              sort:
                - direction: Asc
                  column: 30
                - direction: Desc
                  column: 31
                - direction: Asc
                  column: 32
              tables: []
            span:
              start: 31
              end: 74
          - kind:
              Sort:
                - direction: Asc
                  column:
                    Ident: issued_at
                    ty: Infer
            is_complex: false
            ty:
              columns:
                - All: 29
              sort:
                - direction: Asc
                  column: 33
              tables: []
            span:
              start: 83
              end: 97
          - kind:
              Sort:
                - direction: Desc
                  column:
                    Ident: issued_at
                    ty: Infer
            is_complex: false
            ty:
              columns:
                - All: 29
              sort:
                - direction: Desc
                  column: 33
              tables: []
            span:
              start: 106
              end: 123
          - kind:
              Sort:
                - direction: Asc
                  column:
                    Ident: issued_at
                    ty: Infer
            is_complex: false
            ty:
              columns:
                - All: 29
              sort:
                - direction: Asc
                  column: 33
              tables: []
            span:
              start: 132
              end: 148
          - kind:
              Sort:
                - direction: Desc
                  column:
                    Ident: issued_at
                    ty: Infer
            is_complex: false
            ty:
              columns:
                - All: 29
              sort:
                - direction: Desc
                  column: 33
              tables: []
            span:
              start: 157
              end: 174
        "###);
    }
}
