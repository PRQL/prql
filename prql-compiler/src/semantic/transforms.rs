use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use std::iter::zip;

use crate::ast::pl::fold::{fold_column_sorts, fold_transform_kind, AstFold};
use crate::ast::pl::*;
use crate::error::{Error, Reason, WithErrorInfo};

use super::context::{Decl, DeclKind};
use super::module::{Module, NS_PARAM};
use super::resolver::Resolver;
use super::Frame;

/// try to convert function call with enough args into transform
pub fn cast_transform(resolver: &mut Resolver, closure: Closure) -> Result<Result<Expr, Closure>> {
    let name = closure.name.as_ref().filter(|n| !n.name.contains('.'));
    let name = if let Some(name) = name {
        name.to_string()
    } else {
        return Ok(Err(closure));
    };

    let (kind, input) = match name.as_str() {
        "std.from" => {
            let [source] = unpack::<1>(closure);

            return Ok(Ok(source));
        }
        "std.select" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let assigns = assigns.coerce_into_vec();
            (TransformKind::Select { assigns }, tbl)
        }
        "std.filter" => {
            let [filter, tbl] = unpack::<2>(closure);

            let filter = Box::new(filter);
            (TransformKind::Filter { filter }, tbl)
        }
        "std.derive" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let assigns = assigns.coerce_into_vec();
            (TransformKind::Derive { assigns }, tbl)
        }
        "std.aggregate" => {
            let [assigns, tbl] = unpack::<2>(closure);

            let assigns = assigns.coerce_into_vec();
            (TransformKind::Aggregate { assigns }, tbl)
        }
        "std.sort" => {
            let [by, tbl] = unpack::<2>(closure);

            let by = by
                .coerce_into_vec()
                .into_iter()
                .map(|node| {
                    let (column, direction) = match node.kind {
                        ExprKind::Unary { op, expr } if matches!(op, UnOp::Neg) => {
                            (*expr, SortDirection::Desc)
                        }
                        _ => (node, SortDirection::default()),
                    };

                    ColumnSort { direction, column }
                })
                .collect();

            (TransformKind::Sort { by }, tbl)
        }
        "std.take" => {
            let [expr, tbl] = unpack::<2>(closure);

            let range = match expr.kind {
                ExprKind::Literal(Literal::Integer(n)) => Range::from_ints(None, Some(n)),
                ExprKind::Range(range) => range,
                _ => unimplemented!("`take` range: {expr}"),
            };

            (TransformKind::Take { range }, tbl)
        }
        "std.join" => {
            let [side, with, filter, tbl] = unpack::<4>(closure);

            let side = {
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
            };

            let filter = Box::new(Expr::collect_and(filter.coerce_into_vec()));

            let with = Box::new(with);
            (TransformKind::Join { side, with, filter }, tbl)
        }
        "std.group" => {
            let [by, pipeline, tbl] = unpack::<3>(closure);

            let by = by.coerce_into_vec();

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            let pipeline = Box::new(pipeline);
            (TransformKind::Group { by, pipeline }, tbl)
        }
        "std.window" => {
            let [rows, range, expanding, rolling, pipeline, tbl] = unpack::<6>(closure);

            let expanding = {
                let as_bool = expanding.kind.as_literal().and_then(|l| l.as_boolean());

                *as_bool.ok_or_else(|| {
                    Error::new(Reason::Expected {
                        who: Some("parameter `expanding`".to_string()),
                        expected: "a boolean".to_string(),
                        found: format!("{expanding}"),
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
                        found: format!("{rolling}"),
                    })
                    .with_span(rolling.span)
                })?
            };

            let rows = rows.try_cast(|r| r.into_range(), Some("parameter `rows`"), "a range")?;

            let range = range.try_cast(|r| r.into_range(), Some("parameter `range`"), "a range")?;

            let (kind, range) = if expanding {
                (WindowKind::Rows, Range::from_ints(None, Some(0)))
            } else if rolling > 0 {
                (
                    WindowKind::Rows,
                    Range::from_ints(Some(-rolling + 1), Some(0)),
                )
            } else if !rows.is_empty() {
                (WindowKind::Rows, rows)
            } else if !range.is_empty() {
                (WindowKind::Range, range)
            } else {
                (WindowKind::Rows, Range::unbounded())
            };

            let pipeline = fold_by_simulating_eval(resolver, pipeline, tbl.ty.clone().unwrap())?;

            let transform_kind = TransformKind::Window {
                kind,
                range,
                pipeline: Box::new(pipeline),
            };
            (transform_kind, tbl)
        }
        "std.concat" => {
            let [bottom, top] = unpack::<2>(closure);

            (TransformKind::Concat(Box::new(bottom)), top)
        }
        _ => return Ok(Err(closure)),
    };

    let transform_call = TransformCall {
        kind: Box::new(kind),
        input: Box::new(input),
        partition: Vec::new(),
        frame: WindowFrame::default(),
        sort: Vec::new(),
    };
    Ok(Ok(Expr::from(ExprKind::TransformCall(transform_call))))
}

/// Simulate evaluation of the inner pipeline of group or window
// Creates a dummy node that acts as value that pipeline can be resolved upon.
fn fold_by_simulating_eval(
    resolver: &mut Resolver,
    pipeline: Expr,
    val_type: Ty,
) -> Result<Expr, anyhow::Error> {
    log::debug!("fold by simulating evaluation");

    let param_name = "_tbl";
    let param_id = resolver.id.next().unwrap();

    // resolver will not resolve a function call if any arguments are missing
    // but would instead return a closure to be resolved later.
    // because the pipeline of group is a function that takes a table chunk
    // and applies the transforms to it, it would not get resolved.
    // thats why we trick the resolver with a dummy node that acts as table
    // chunk and instruct resolver to apply the transform on that.

    let mut dummy = Expr::from(ExprKind::Ident(Ident::from_name(param_name)));
    dummy.ty = Some(val_type);

    let pipeline = Expr::from(ExprKind::FuncCall(FuncCall {
        name: Box::new(pipeline),
        args: vec![dummy],
        named_args: Default::default(),
    }));

    let env = Module::singleton(param_name, Decl::from(DeclKind::Column(param_id)));
    resolver.decls.root_mod.stack_push(NS_PARAM, env);

    let pipeline = resolver.fold_expr(pipeline)?;

    resolver.decls.root_mod.stack_pop(NS_PARAM).unwrap();

    // now, we need wrap the result into a closure and replace
    // the dummy node with closure's parameter.

    // extract reference to the dummy node
    // let mut tbl_node = extract_ref_to_first(&mut pipeline);
    // *tbl_node = Expr::from(ExprKind::Ident("x".to_string()));

    let pipeline = Expr::from(ExprKind::Closure(Box::new(Closure {
        name: None,
        body: Box::new(pipeline),
        body_ty: None,

        args: vec![],
        params: vec![FuncParam {
            name: param_id.to_string(),
            ty: None,
            default_value: None,
        }],
        named_params: vec![],

        env: Default::default(),
    })));
    Ok(pipeline)
}

impl TransformCall {
    pub fn infer_type(&self) -> Result<Frame> {
        use TransformKind::*;

        fn ty_frame_or_default(expr: &Expr) -> Result<Frame> {
            expr.ty
                .as_ref()
                .and_then(|t| t.as_table())
                .cloned()
                .ok_or_else(|| anyhow!("expected {expr:?} to have table type"))
        }

        Ok(match self.kind.as_ref() {
            Select { assigns } => {
                let mut frame = ty_frame_or_default(&self.input)?;

                frame.columns.clear();
                frame.apply_assigns(assigns);
                frame
            }
            Derive { assigns } => {
                let mut frame = ty_frame_or_default(&self.input)?;

                frame.apply_assigns(assigns);
                frame
            }
            Group { pipeline, by, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap().as_ref();

                let mut frame = body.ty.clone().unwrap().into_table().unwrap();

                log::debug!("inferring type of group with pipeline: {body}");

                // prepend aggregate with `by` columns
                if let ExprKind::TransformCall(TransformCall { kind, .. }) = &body.as_ref().kind {
                    if let TransformKind::Aggregate { .. } = kind.as_ref() {
                        let aggregate_columns = frame.columns;
                        frame.columns = Vec::new();

                        log::debug!(".. group by {by:?}");
                        frame.apply_assigns(by);

                        frame.columns.extend(aggregate_columns);
                    }
                }

                log::debug!(".. type={frame}");

                frame
            }
            Window { pipeline, .. } => {
                // pipeline's body is resolved, just use its type
                let Closure { body, .. } = pipeline.kind.as_closure().unwrap().as_ref();

                body.ty.clone().unwrap().into_table().unwrap()
            }
            Aggregate { assigns } => {
                let mut frame = ty_frame_or_default(&self.input)?;
                frame.columns.clear();

                frame.apply_assigns(assigns);
                frame
            }
            Join { with, .. } => {
                let left = ty_frame_or_default(&self.input)?;
                let right = ty_frame_or_default(with)?;
                join(left, right)
            }
            Concat(bottom) => {
                let top = ty_frame_or_default(&self.input)?;
                let bottom = ty_frame_or_default(bottom)?;
                concat(top, bottom)?
            }
            Sort { .. } | Filter { .. } | Take { .. } => ty_frame_or_default(&self.input)?,
        })
    }
}

fn join(mut lhs: Frame, rhs: Frame) -> Frame {
    lhs.columns.extend(rhs.columns);
    lhs.inputs.extend(rhs.inputs);
    lhs
}

fn concat(mut top: Frame, bottom: Frame) -> Result<Frame, Error> {
    if top.columns.len() != bottom.columns.len() {
        return Err(Error::new(Reason::Simple(
            "cannot concat two relations with non-matching number of columns.".to_string(),
        )))
        .with_help(format!(
            "top has {} columns, but bottom has {}",
            top.columns.len(),
            bottom.columns.len()
        ));
    }

    // TODO: I'm not sure what to use as input_name and expr_id...
    let mut columns = Vec::with_capacity(top.columns.len());
    for (t, b) in zip(top.columns, bottom.columns) {
        columns.push(match (t, b) {
            (FrameColumn::Wildcard { input_name }, FrameColumn::Wildcard { .. }) => {
                FrameColumn::Wildcard { input_name }
            }
            (
                FrameColumn::Single {
                    name: name_t,
                    expr_id,
                },
                FrameColumn::Single { name: name_b, .. },
            ) => match (name_t, name_b) {
                (None, None) => {
                    let name = None;
                    FrameColumn::Single { name, expr_id }
                }
                (None, Some(name)) | (Some(name), _) => {
                    let name = Some(name);
                    FrameColumn::Single { name, expr_id }
                }
            },
            (t, b) => {
                return Err(Error::new(Reason::Simple(format!(
                    "cannot match columns `{t:?}` and `{b:?}`"
                )))
                .with_help(
                    "make sure that top and bottom relations of concat has the same column layout",
                ))
            }
        });
    }

    top.columns = columns;
    Ok(top)
}

fn unpack<const P: usize>(closure: Closure) -> [Expr; P] {
    closure.args.try_into().expect("bad transform cast")
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
    replace_map: HashMap<usize, Expr>,
}

impl Flattener {
    pub fn fold(expr: Expr) -> Expr {
        let mut f = Flattener::default();
        f.fold_expr(expr).unwrap()
    }
}

impl AstFold for Flattener {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        if let Some(target) = &expr.target_id {
            if let Some(replacement) = self.replace_map.remove(target) {
                return Ok(replacement);
            }
        }

        expr.kind = match expr.kind {
            ExprKind::TransformCall(t) => {
                log::debug!("flattening {}", (*t.kind).as_ref());

                let (input, kind) = match *t.kind {
                    TransformKind::Sort { by } => {
                        // fold
                        let by = fold_column_sorts(self, by)?;
                        let input = self.fold_expr(*t.input)?;

                        self.sort = by.clone();

                        if self.sort_undone {
                            return Ok(input);
                        } else {
                            (input, TransformKind::Sort { by })
                        }
                    }
                    TransformKind::Group { by, pipeline } => {
                        let sort_undone = self.sort_undone;
                        self.sort_undone = true;

                        let input = self.fold_expr(*t.input)?;

                        let pipeline = pipeline.kind.into_closure().unwrap();

                        let table_param = &pipeline.params[0];
                        let param_id = table_param.name.parse::<usize>().unwrap();

                        self.replace_map.insert(param_id, input);
                        self.partition = by;
                        self.sort.clear();

                        let pipeline = self.fold_expr(*pipeline.body)?;

                        self.replace_map.remove(&param_id);
                        self.partition.clear();
                        self.sort.clear();
                        self.sort_undone = sort_undone;

                        return Ok(Expr {
                            ty: expr.ty,
                            ..pipeline
                        });
                    }
                    TransformKind::Window {
                        kind,
                        range,
                        pipeline,
                    } => {
                        let tbl = self.fold_expr(*t.input)?;
                        let pipeline = pipeline.kind.into_closure().unwrap();

                        let table_param = &pipeline.params[0];
                        let param_id = table_param.name.parse::<usize>().unwrap();

                        self.replace_map.insert(param_id, tbl);
                        self.window = WindowFrame { kind, range };

                        let pipeline = self.fold_expr(*pipeline.body)?;

                        self.window = WindowFrame::default();
                        self.replace_map.remove(&param_id);

                        return Ok(Expr {
                            ty: expr.ty,
                            ..pipeline
                        });
                    }
                    kind => (self.fold_expr(*t.input)?, fold_transform_kind(self, kind)?),
                };

                ExprKind::TransformCall(TransformCall {
                    input: Box::new(input),
                    kind: Box::new(kind),
                    partition: self.partition.clone(),
                    frame: self.window.clone(),
                    sort: self.sort.clone(),
                })
            }
            kind => self.fold_expr_kind(kind)?,
        };
        Ok(expr)
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_yaml_snapshot;

    use crate::parse;
    use crate::semantic::{resolve, resolve_only};

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
        let result = resolve(query).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        def:
          version: ~
          dialect: Generic
        tables:
          - id: 0
            name: c_invoice
            relation:
              kind:
                ExternRef:
                  LocalTable: c_invoice
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
        let (result, _) = resolve_only(query, None).unwrap();
        assert_yaml_snapshot!(result, @r###"
        ---
        - Pipeline:
            id: 12
            TransformCall:
              input:
                id: 4
                Ident:
                  - default_db
                  - c_invoice
                ty:
                  Table:
                    columns:
                      - Wildcard:
                          input_name: c_invoice
                    inputs:
                      - id: 4
                        name: c_invoice
                        table:
                          - default_db
                          - c_invoice
              kind:
                Aggregate:
                  assigns:
                    - id: 18
                      SString:
                        - String: AVG(
                        - Expr:
                            id: 17
                            Ident:
                              - _frame
                              - c_invoice
                              - amount
                            target_id: 4
                            ty: Infer
                        - String: )
                      ty:
                        Literal: Column
              partition:
                - id: 8
                  Ident:
                    - _frame
                    - c_invoice
                    - date
                  target_id: 4
                  ty: Infer
            ty:
              Table:
                columns:
                  - Single:
                      name:
                        - date
                      expr_id: 8
                  - Single:
                      name: ~
                      expr_id: 18
                inputs:
                  - id: 4
                    name: c_invoice
                    table:
                      - default_db
                      - c_invoice
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
            relation:
              kind:
                ExternRef:
                  LocalTable: invoices
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
