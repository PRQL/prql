use std::collections::HashMap;

use crate::ir::pl::{fold_column_sorts, fold_transform_kind};
use crate::ir::pl::{
    ColumnSort, Expr, ExprKind, PlFold, TransformCall, TransformKind, WindowFrame,
};
use crate::semantic::NS_LOCAL;
use crate::Result;

/// Flattens group and window [TransformCall]s into a single pipeline.
/// Sets partition, window and sort of [TransformCall].
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
    partition: Option<Box<Expr>>,

    /// Window affects transforms in it's inner pipeline.
    /// This means that this field has to be set before folding inner pipeline,
    /// and unset after the folding.
    window: WindowFrame,

    /// Window and group contain Closures in their inner pipelines.
    /// These closures have form similar to this function:
    /// ```prql
    /// let closure = tbl_chunk -> (derive ... (sort ... (tbl_chunk)))
    /// ```
    /// To flatten a window or group, we need to replace group/window transform
    /// with their closure's body and replace `tbl_chunk` with pipeline
    /// preceding the group/window transform.
    ///
    /// That's what `replace_map` is for.
    replace_map: HashMap<String, Expr>,
}

impl Flattener {
    pub fn run(expr: Expr) -> Result<Expr> {
        let mut f = Flattener {
            sort: Default::default(),
            sort_undone: Default::default(),
            partition: Default::default(),
            window: Default::default(),
            replace_map: Default::default(),
        };
        f.fold_expr(expr)
    }
}

impl PlFold for Flattener {
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        if let ExprKind::Ident(fq_ident) = &expr.kind {
            if fq_ident.starts_with_part(NS_LOCAL) && fq_ident.len() == 2 {
                if let Some(replacement) = self.replace_map.remove(&fq_ident.name) {
                    return Ok(replacement);
                }
            }
        }

        if let ExprKind::RqOperator { name, .. } = &expr.kind {
            if !name.starts_with("std.") {
                expr = super::special_functions::resolve_special_func(expr)?
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

                        let pipeline = pipeline.kind.into_func().unwrap();

                        let table_param = &pipeline.params[0];

                        self.replace_map.insert(table_param.name.clone(), input);
                        self.partition = Some(by);
                        self.sort.clear();

                        let pipeline = self.fold_expr(*pipeline.body)?;

                        self.replace_map.remove(&table_param.name);
                        self.partition = None;
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
                        let pipeline = pipeline.kind.into_func().unwrap();

                        let table_param = &pipeline.params[0];

                        self.replace_map.insert(table_param.name.clone(), tbl);
                        self.window = WindowFrame { kind, range };

                        let pipeline = self.fold_expr(*pipeline.body)?;

                        self.window = WindowFrame::default();
                        self.replace_map.remove(&table_param.name);

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
