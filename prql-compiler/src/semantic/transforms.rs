use anyhow::{anyhow, bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::AstFold;
use crate::ast::*;
use crate::error::{Error, Reason};

use super::resolver::Resolver;
use super::Frame;

/// try to convert function call with enough args into transform
pub fn cast_transform(
    resolver: &mut Resolver,
    closure: Closure,
) -> Result<Result<TransformCall, Closure>> {
    // TODO: We don't want to match transforms by name.
    // Add `builtin` parse derivation that produces a special AST node.
    // This node can then matched here instead of matching a string function name.

    let kind = match closure.name.as_deref().unwrap_or("") {
        "from" => {
            let ([source], []) = unpack::<1, 0>(closure)?;

            TransformKind::From(source)
        }
        "select" => {
            let ([assigns, tbl], []) = unpack::<2, 0>(closure)?;

            let mut assigns = assigns.coerce_into_vec();
            resolver.context.declare_as_idents(&mut assigns);

            TransformKind::Select { assigns, tbl }
        }
        "filter" => {
            let ([filter, tbl], []) = unpack::<2, 0>(closure)?;

            let filter = Box::new(filter);
            TransformKind::Filter { filter, tbl }
        }
        "derive" => {
            let ([assigns, tbl], []) = unpack::<2, 0>(closure)?;

            let mut assigns = assigns.coerce_into_vec();
            resolver.context.declare_as_idents(&mut assigns);

            TransformKind::Derive { assigns, tbl }
        }
        "aggregate" => {
            let ([assigns, tbl], []) = unpack::<2, 0>(closure)?;

            let mut assigns = assigns.coerce_into_vec();
            resolver.context.declare_as_idents(&mut assigns);

            TransformKind::Aggregate { assigns, tbl }
        }
        "sort" => {
            let ([by, tbl], []) = unpack::<2, 0>(closure)?;

            let by = by
                .coerce_into_vec()
                .into_iter()
                .map(|node| {
                    let (mut column, direction) = match node.kind {
                        ExprKind::Unary { op, expr } if matches!(op, UnOp::Neg) => {
                            (*expr, SortDirection::Desc)
                        }
                        _ => (node, SortDirection::default()),
                    };

                    resolver.context.declare_as_ident(&mut column);

                    ColumnSort { direction, column }
                })
                .collect();

            TransformKind::Sort { by, tbl }
        }
        "take" => {
            let ([expr, tbl], []) = unpack::<2, 0>(closure)?;

            let range = match expr.kind {
                ExprKind::Literal(Literal::Integer(n)) => Range::from_ints(None, Some(n)),
                ExprKind::Range(range) => range,
                _ => unimplemented!("`take` range: {expr}"),
            };

            TransformKind::Take { range, tbl }
        }
        "join" => {
            let ([with, filter, tbl], [side]) = unpack::<3, 1>(closure)?;

            let side = if let Some(side) = side {
                let span = side.span;
                let ident = side.try_cast(ExprKind::into_ident, Some("side"), "ident")?;
                match ident.to_string().as_str() {
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

            let filter = Box::new(Expr::collect_and(filter.coerce_into_vec()));

            let with = Box::new(with);
            TransformKind::Join {
                side,
                with,
                filter,
                tbl,
            }
        }
        "group" => {
            let ([by, pipeline, tbl], []) = unpack::<3, 0>(closure)?;

            let by = by
                .coerce_into_vec()
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

            // resolver will not resolve a function call if any arguments are missing but
            // would instead return a closure to be resolved later.
            // because the pipeline of group is a function that takes a table chunk and applies
            // the transforms to it, it would not get resolved.
            // thats why we trick the resolver with a dummy node that acts as table chunk and
            // instruct resolver to apply the transform on that.

            // TODO: having dummy already be `x` is a hack.
            // Dummy should be substituted in later.
            let mut dummy = Expr::from(ExprKind::Ident(IdentWithNamespace {
                namespace: None,
                ident: ("_x".to_string()),
            }));
            dummy.ty = tbl.ty.clone();

            let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(pipeline),
                args: vec![dummy],
                named_args: Default::default(),
            }));
            let pipeline = resolver.fold_expr(pipeline)?;

            // now, we need wrap the result into a closure and replace the dummy node with closure's parameter.

            // extract reference to the dummy node
            // let mut tbl_node = extract_ref_to_first(&mut pipeline);
            // *tbl_node = Expr::from(ExprKind::Ident("x".to_string()));

            let pipeline = Expr::from(ExprKind::Closure(Closure {
                name: None,
                body: Box::new(pipeline),

                args: vec![],
                params: vec![FuncParam {
                    name: "_x".to_string(),
                    ty: None,
                    default_value: None,
                }],

                named_args: vec![],
                named_params: vec![],

                env: Default::default(),
            }));

            let pipeline = Box::new(pipeline);
            TransformKind::Group { by, pipeline, tbl }
        }
        "window" => {
            let ([pipeline, tbl], [rows, range, expanding, rolling]) = unpack::<2, 4>(closure)?;

            let expanding = if let Some(expanding) = expanding {
                let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: format!("{expanding}"),
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
                        found: format!("{rolling}"),
                    })
                    .with_span(rolling.span)
                })?
            } else {
                0
            };

            let rows = if let Some(rows) = rows {
                Some(rows.try_cast(|r| r.into_range(), Some("parameter `rows`"), "a range")?)
            } else {
                None
            };

            let range = if let Some(range) = range {
                Some(range.try_cast(|r| r.into_range(), Some("parameter `range`"), "a range")?)
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
            let mut value = Expr::from(ExprKind::Literal(Literal::Null));
            value.ty = tbl.ty.clone();

            let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
                name: Box::new(pipeline),
                args: vec![value],
                named_args: Default::default(),
            }));
            let pipeline = Box::new(resolver.fold_expr(pipeline)?);

            TransformKind::Window {
                kind,
                range,
                pipeline,
                tbl,
            }
        }
        _ => return Ok(Err(closure)),
    };

    Ok(Ok(TransformCall::from(kind)))
}

impl TransformCall {
    pub fn infer_type(&self) -> Result<Frame> {
        use TransformKind::*;

        fn ty_frame_or_default(expr: &Expr) -> Frame {
            expr.ty
                .as_ref()
                .and_then(|t| t.as_table())
                .cloned()
                .unwrap_or_default()
        }

        Ok(match self.kind.as_ref() {
            From(t) => match &t.ty {
                Some(Ty::Table(f)) => f.clone(),
                Some(ty) => bail!("`from` expected a table name, got `{t}` of type `{ty}`"),
                a => bail!("`from` expected a frame got `{t:?}` of type {a:?}"),
            },

            Select { assigns, tbl } => {
                let mut frame = ty_frame_or_default(tbl);

                frame.columns.clear();
                frame.apply_assigns(assigns);
                frame
            }
            Derive { assigns, tbl } => {
                let mut frame = ty_frame_or_default(tbl);

                frame.apply_assigns(assigns);
                frame
            }
            Group { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap();

                body.ty.clone().unwrap().into_table().unwrap()
            }
            Window { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap();

                body.ty.clone().unwrap().into_table().unwrap()
            }
            Aggregate { assigns, tbl } => {
                let mut frame = ty_frame_or_default(tbl);

                // let old_columns = frame.columns.clone();

                frame.columns.clear();

                // TODO: add `by` columns into frame when aggregate is within group
                // for b in by {
                //     let id = b.declared_at.unwrap();
                //     let col = old_columns.iter().find(|c| c == &&id);
                //     let name = col.and_then(|c| match c {
                //         FrameColumn::Named(n, _) => Some(n.clone()),
                //         _ => None,
                //     });

                //     frame.push_column(name, id);
                // }

                frame.apply_assigns(assigns);
                frame
            }
            Join { tbl, with, .. } => {
                let mut frame = ty_frame_or_default(tbl);

                let table_id = with
                    .declared_at
                    .ok_or_else(|| anyhow!("unresolved table {with:?}"))?;
                frame.tables.push(table_id);
                frame.columns.push(FrameColumn::All(table_id));

                frame
            }
            Sort { by, tbl } => {
                let mut frame = ty_frame_or_default(tbl);

                frame.sort = extract_sorts(by)?;
                frame
            }
            Filter { tbl, .. } | Take { tbl, .. } => ty_frame_or_default(tbl),
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

fn unpack<const P: usize, const N: usize>(
    closure: Closure,
) -> Result<([Expr; P], [Option<Expr>; N])> {
    let named = closure
        .named_args
        .try_into()
        .unwrap_or_else(|na| panic!("bad transform cast: {:?} {na:?}", closure.name));
    let positional = closure.args.try_into().expect("bad transform cast");

    Ok((positional, named))
}

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

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::{parse, semantic::resolve};

    #[test]
    #[should_panic]
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
        let (result, _) = resolve(query).unwrap();
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
            partition: []
            window: ~
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
                    Literal: Column
            is_complex: false
            partition: []
            window: ~
            ty:
              columns:
                - Named:
                    - invoice_no
                    - 30
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
                    partition: []
                    window: ~
                    ty:
                      columns:
                        - Named:
                            - invoice_no
                            - 30
                      sort: []
                      tables: []
                    span: ~
            is_complex: false
            partition: []
            window: ~
            ty:
              columns:
                - Named:
                    - invoice_no
                    - 30
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
        let result = resolve(query);
        assert!(result.is_err());

        // oops, two arguments
        let query = parse(
            "
        from c_invoice
        group date (aggregate average amount)
        ",
        )
        .unwrap();
        let result = resolve(query);
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
        let (result, _) = resolve(query).unwrap();
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
            partition: []
            window: ~
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
                          - SString:
                              - String: AVG(
                              - Expr:
                                  Ident: amount
                                  ty: Infer
                              - String: )
                            ty:
                              Literal: Column
                        by: []
                    is_complex: false
                    partition: []
                    window: ~
                    ty:
                      columns:
                        - Unnamed: 32
                      sort: []
                      tables: []
                    span: ~
            is_complex: false
            partition: []
            window: ~
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

        let (result, _) = resolve(query).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables:
          - id: 0
            name: invoices
            expr:
              Ref:
                LocalTable: invoices
        expr:
          Pipeline:
            - From: 0
            - Derive:
                id: 4
                name: issued_at
                expr:
                  kind:
                    ExternRef:
                      variable: issued_at
                      table: ~
                  span:
                    start: 37
                    end: 46
            - Derive:
                id: 5
                name: amount
                expr:
                  kind:
                    ExternRef:
                      variable: amount
                      table: ~
                  span:
                    start: 49
                    end: 55
            - Derive:
                id: 6
                name: num_of_articles
                expr:
                  kind:
                    ExternRef:
                      variable: num_of_articles
                      table: ~
                  span:
                    start: 57
                    end: 73
            - Sort:
                - direction: Asc
                  column: 4
                - direction: Desc
                  column: 5
                - direction: Asc
                  column: 6
            - Derive:
                id: 3
                name: issued_at
                expr:
                  kind:
                    ExternRef:
                      variable: issued_at
                      table: ~
                  span:
                    start: 88
                    end: 97
            - Sort:
                - direction: Asc
                  column: 3
            - Derive:
                id: 2
                name: issued_at
                expr:
                  kind:
                    ExternRef:
                      variable: issued_at
                      table: ~
                  span:
                    start: 113
                    end: 122
            - Sort:
                - direction: Desc
                  column: 2
            - Derive:
                id: 1
                name: issued_at
                expr:
                  kind:
                    ExternRef:
                      variable: issued_at
                      table: ~
                  span:
                    start: 138
                    end: 147
            - Sort:
                - direction: Asc
                  column: 1
            - Derive:
                id: 0
                name: issued_at
                expr:
                  kind:
                    ExternRef:
                      variable: issued_at
                      table: ~
                  span:
                    start: 164
                    end: 173
            - Sort:
                - direction: Desc
                  column: 0
        "###);
    }
}
