use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use itertools::Itertools;

use crate::ast::ast_fold::{fold_column_sorts, fold_transform_kind, AstFold};
use crate::ast::*;
use crate::error::{Error, Reason};

use super::resolver::Resolver;
use super::{Declaration, Frame};

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

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

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

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            TransformKind::Window {
                kind,
                range,
                pipeline: Box::new(pipeline),
                tbl,
            }
        }
        _ => return Ok(Err(closure)),
    };

    Ok(Ok(TransformCall::from(kind)))
}

/// Simulate evaluation of the inner pipeline of group or window
// Creates a dummy node that acts as value that pipeline can be resolved upon.
fn fold_by_simulating_eval(
    resolver: &mut Resolver,
    pipeline: Expr,
    val_type: Ty,
) -> Result<Expr, anyhow::Error> {
    // this is a workaround for having unique names
    let param_name = {
        let a_unique_number = resolver.context.declare(
            Declaration::Expression(Box::new(Expr::from(ExprKind::Literal(Literal::Null)))),
            None,
        );

        format!("_tbl_{}", a_unique_number)
    };

    // resolver will not resolve a function call if any arguments are missing
    // but would instead return a closure to be resolved later.
    // because the pipeline of group is a function that takes a table chunk
    // and applies the transforms to it, it would not get resolved.
    // thats why we trick the resolver with a dummy node that acts as table
    // chunk and instruct resolver to apply the transform on that.

    // TODO: having dummy already be `x` is a hack.
    // Dummy should be substituted in later.
    let mut dummy = Expr::from(ExprKind::Ident(Ident::from_name(param_name.clone())));
    dummy.ty = Some(val_type);
    let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
        name: Box::new(pipeline),
        args: vec![dummy],
        named_args: Default::default(),
    }));
    let pipeline = resolver.fold_expr(pipeline)?;

    // now, we need wrap the result into a closure and replace
    // the dummy node with closure's parameter.

    // extract reference to the dummy node
    // let mut tbl_node = extract_ref_to_first(&mut pipeline);
    // *tbl_node = Expr::from(ExprKind::Ident("x".to_string()));

    let pipeline = Expr::from(ExprKind::Closure(Closure {
        name: None,
        body: Box::new(pipeline),
        body_ty: None,

        args: vec![],
        params: vec![FuncParam {
            name: param_name,
            ty: None,
            default_value: None,
        }],

        named_args: vec![],
        named_params: vec![],

        env: Default::default(),
    }));
    Ok(pipeline)
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
            Group { pipeline, by, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap();

                let mut frame = body.ty.clone().unwrap().into_table().unwrap();

                // prepend aggregate with `by` columns
                if let ExprKind::TransformCall(TransformCall { kind, .. }) = &body.as_ref().kind {
                    if let TransformKind::Aggregate { .. } = kind.as_ref() {
                        let aggregate_columns = frame.columns;
                        frame.columns = Vec::new();
                        for b in by {
                            let id = b.declared_at.unwrap();
                            let name = b.alias.clone().or_else(|| match &b.kind {
                                ExprKind::Ident(ident) => Some(ident.name.clone()),
                                _ => None,
                            });

                            frame.push_column(name, id);
                        }

                        frame.columns.extend(aggregate_columns);
                    }
                }

                frame
            }
            Window { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap();

                body.ty.clone().unwrap().into_table().unwrap()
            }
            Aggregate { assigns, tbl } => {
                let mut frame = ty_frame_or_default(tbl);
                frame.columns.clear();

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

/// Flattens group and window [TransformCall]s into a single pipeline.
/// Sets partition, window and sort of [TransformCall].
#[derive(Default)]
pub struct Flattener {
    /// Sort affects downstream transforms in a pipeline.
    /// Because transform pipelines are represented by nested [TransformCall]s,
    /// affected transforms are all ancestor nodes of sort [TransformCall].
    /// This means that this field has to be set after folding inner table,
    /// so it's passed to parent call of `fold_transform_call`
    sort: Vec<ColumnSort>,

    sort_undone: bool,

    /// Group affects transforms in it's inner pipeline.
    /// This means that this field has to be set before folding inner pipeline,
    /// and unset after the folding.
    partition: Vec<Expr>,

    /// Window affects transforms in it's inner pipeline.
    /// This means that this field has to be set before folding inner pipeline,
    /// and unset after the folding.
    window: WindowFrame,

    /// Window and group contain Closures in their inner pipelines.
    /// These closures have form similar to this function:
    /// ```
    /// func closure tbl_chunk -> (derive ... (sort ... (tbl_chunk)))
    /// ```
    /// To flatten a window or group, we need to replace group/window transform
    /// with their closure's body and replace `tbl_chunk` with pipeline
    /// preceding the group/window transform.
    ///
    /// That's what `replace_map` is for.
    replace_map: HashMap<String, Expr>,
}

impl AstFold for Flattener {
    fn fold_transform_call(&mut self, t: TransformCall) -> Result<TransformCall> {
        let kind = match *t.kind {
            TransformKind::Sort { by, tbl } => {
                // fold
                let by = fold_column_sorts(self, by)?;
                let tbl = self.fold_expr(tbl)?;

                self.sort = by.clone();

                if self.sort_undone {
                    return Ok(tbl.kind.into_transform_call().unwrap());
                } else {
                    TransformKind::Sort { by, tbl }
                }
            }
            TransformKind::Group { by, pipeline, tbl } => {
                let sort_undone = self.sort_undone;
                self.sort_undone = true;

                let tbl = self.fold_expr(tbl)?;

                let pipeline = pipeline.kind.into_closure().unwrap();

                let table_param = &pipeline.params[0];

                self.replace_map.insert(table_param.name.clone(), tbl);
                self.partition = by;

                let expr = self.fold_expr(*pipeline.body)?;

                self.partition = Vec::new();
                self.replace_map.remove(&table_param.name);
                self.sort.clear();
                self.sort_undone = sort_undone;

                return Ok(expr.kind.into_transform_call().unwrap());
            }
            TransformKind::Window {
                kind,
                range,
                pipeline,
                tbl,
            } => {
                let tbl = self.fold_expr(tbl)?;
                let pipeline = pipeline.kind.into_closure().unwrap();

                let table_param = &pipeline.params[0];

                self.replace_map.insert(table_param.name.clone(), tbl);
                self.window = WindowFrame { kind, range };

                let expr = self.fold_expr(*pipeline.body)?;

                self.window = WindowFrame::default();
                self.replace_map.remove(&table_param.name);

                return Ok(expr.kind.into_transform_call().unwrap());
            }
            kind => fold_transform_kind(self, kind)?,
        };

        Ok(TransformCall {
            kind: Box::new(kind),
            partition: self.partition.clone(),
            frame: self.window.clone(),
            sort: self.sort.clone(),
        })
    }

    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        if let ExprKind::Ident(ident) = &expr.kind {
            if ident.namespace.is_none() {
                if let Some(replacement) = self.replace_map.remove(&ident.name) {
                    return Ok(replacement);
                }
            }
        }

        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::{parse, semantic::resolve};

    #[test]
    #[ignore]
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
        let result = resolve(query).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables:
          - id: 0
            name: c_invoice
            expr:
              ExternRef:
                - LocalTable: c_invoice
                - - id: 1
                    kind: Wildcard
                  - id: 0
                    kind:
                      ExternRef: invoice_no
        expr:
          Pipeline:
            - From:
                source: 0
                columns:
                  - id: 3
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 1
                          span: ~
                  - id: 4
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 0
                          span: ~
                name: ~
            - Select:
                - 4
            - Take:
                start: ~
                end:
                  kind:
                    Literal:
                      Integer: 1
                  span: ~
            - Select:
                - 4
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
        let result = resolve(query).unwrap();
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

        let result = resolve(query).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables:
          - id: 0
            name: invoices
            expr:
              ExternRef:
                - LocalTable: invoices
                - - id: 1
                    kind: Wildcard
                  - id: 0
                    kind:
                      ExternRef: issued_at
                  - id: 6
                    kind:
                      ExternRef: amount
                  - id: 7
                    kind:
                      ExternRef: num_of_articles
        expr:
          Pipeline:
            - From:
                source: 0
                columns:
                  - id: 15
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 1
                          span: ~
                  - id: 16
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 0
                          span: ~
                  - id: 17
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 6
                          span: ~
                  - id: 18
                    kind:
                      Expr:
                        name: ~
                        expr:
                          kind:
                            ColumnRef: 7
                          span: ~
                name: ~
            - Sort:
                - direction: Asc
                  column: 16
                - direction: Desc
                  column: 17
                - direction: Asc
                  column: 18
            - Sort:
                - direction: Asc
                  column: 16
            - Sort:
                - direction: Desc
                  column: 16
            - Sort:
                - direction: Asc
                  column: 16
            - Sort:
                - direction: Desc
                  column: 16
            - Select:
                - 15
        "###);
    }
}
