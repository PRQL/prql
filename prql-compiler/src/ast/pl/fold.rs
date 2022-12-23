/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;

use super::*;

// Fold pattern:
// - https://rust-unofficial.github.io/patterns/patterns/creational/fold.html
// Good discussions on the visitor / fold pattern:
// - https://github.com/rust-unofficial/patterns/discussions/236 (within this,
//   this comment looked interesting: https://github.com/rust-unofficial/patterns/discussions/236#discussioncomment-393517)
// - https://news.ycombinator.com/item?id=25620110

// For some functions, we want to call a default impl, because copying &
// pasting everything apart from a specific match is lots of repetition. So
// we define a function outside the trait, by default call it, and let
// implementors override the default while calling the function directly for
// some cases. Ref https://stackoverflow.com/a/66077767/3064736
pub trait AstFold {
    fn fold_stmt(&mut self, mut stmt: Stmt) -> Result<Stmt> {
        stmt.kind = fold_stmt_kind(self, stmt.kind)?;
        Ok(stmt)
    }
    fn fold_stmts(&mut self, stmts: Vec<Stmt>) -> Result<Vec<Stmt>> {
        stmts.into_iter().map(|stmt| self.fold_stmt(stmt)).collect()
    }
    fn fold_expr(&mut self, mut expr: Expr) -> Result<Expr> {
        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
    fn fold_expr_kind(&mut self, expr_kind: ExprKind) -> Result<ExprKind> {
        fold_expr_kind(self, expr_kind)
    }
    fn fold_exprs(&mut self, exprs: Vec<Expr>) -> Result<Vec<Expr>> {
        exprs.into_iter().map(|node| self.fold_expr(node)).collect()
    }
    fn fold_table(&mut self, table: TableDef) -> Result<TableDef> {
        Ok(TableDef {
            name: table.name,
            value: Box::new(self.fold_expr(*table.value)?),
        })
    }
    fn fold_pipeline(&mut self, pipeline: Pipeline) -> Result<Pipeline> {
        fold_pipeline(self, pipeline)
    }
    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        fold_func_def(self, function)
    }
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_call)
    }
    fn fold_transform_call(&mut self, transform_call: TransformCall) -> Result<TransformCall> {
        fold_transform_call(self, transform_call)
    }
    fn fold_closure(&mut self, closure: Closure) -> Result<Closure> {
        fold_closure(self, closure)
    }
    fn fold_interpolate_item(&mut self, sstring_item: InterpolateItem) -> Result<InterpolateItem> {
        fold_interpolate_item(self, sstring_item)
    }
    fn fold_type(&mut self, t: Ty) -> Result<Ty> {
        fold_type(self, t)
    }
    fn fold_window(&mut self, window: WindowFrame) -> Result<WindowFrame> {
        fold_window(self, window)
    }
}

pub fn fold_expr_kind<T: ?Sized + AstFold>(fold: &mut T, expr_kind: ExprKind) -> Result<ExprKind> {
    use ExprKind::*;
    Ok(match expr_kind {
        Ident(ident) => Ident(ident),
        Binary { op, left, right } => Binary {
            op,
            left: Box::new(fold.fold_expr(*left)?),
            right: Box::new(fold.fold_expr(*right)?),
        },
        Unary { op, expr } => Unary {
            op,
            expr: Box::new(fold.fold_expr(*expr)?),
        },
        List(items) => List(fold.fold_exprs(items)?),
        Range(range) => Range(fold_range(fold, range)?),
        Pipeline(p) => Pipeline(fold.fold_pipeline(p)?),
        SString(items) => SString(
            items
                .into_iter()
                .map(|x| fold.fold_interpolate_item(x))
                .try_collect()?,
        ),
        FString(items) => FString(
            items
                .into_iter()
                .map(|x| fold.fold_interpolate_item(x))
                .try_collect()?,
        ),
        Switch(cases) => Switch(fold_cases(fold, cases)?),
        Match(expr, cases) => Match(Box::new(fold.fold_expr(*expr)?), fold_cases(fold, cases)?),

        FuncCall(func_call) => FuncCall(fold.fold_func_call(func_call)?),
        Closure(closure) => Closure(Box::new(fold.fold_closure(*closure)?)),

        TransformCall(transform) => TransformCall(fold.fold_transform_call(transform)?),
        BuiltInFunction { name, args } => BuiltInFunction {
            name,
            args: fold.fold_exprs(args)?,
        },

        // None of these capture variables, so we don't need to fold them.
        Literal(_) => expr_kind,
    })
}

pub fn fold_stmt_kind<T: ?Sized + AstFold>(fold: &mut T, stmt_kind: StmtKind) -> Result<StmtKind> {
    use StmtKind::*;
    Ok(match stmt_kind {
        FuncDef(func) => FuncDef(fold.fold_func_def(func)?),
        TableDef(table) => TableDef(fold.fold_table(table)?),
        Main(expr) => Main(Box::new(fold.fold_expr(*expr)?)),
        QueryDef(_) => stmt_kind,
    })
}

pub fn fold_window<F: ?Sized + AstFold>(fold: &mut F, window: WindowFrame) -> Result<WindowFrame> {
    Ok(WindowFrame {
        kind: window.kind,
        range: fold_range(fold, window.range)?,
    })
}

pub fn fold_range<F: ?Sized + AstFold>(fold: &mut F, Range { start, end }: Range) -> Result<Range> {
    Ok(Range {
        start: fold_optional_box(fold, start)?,
        end: fold_optional_box(fold, end)?,
    })
}

pub fn fold_pipeline<T: ?Sized + AstFold>(fold: &mut T, pipeline: Pipeline) -> Result<Pipeline> {
    Ok(Pipeline {
        exprs: fold.fold_exprs(pipeline.exprs)?,
    })
}

// This aren't strictly in the hierarchy, so we don't need to
// have an assoc. function for `fold_optional_box` — we just
// call out to the function in this module
pub fn fold_optional_box<F: ?Sized + AstFold>(
    fold: &mut F,
    opt: Option<Box<Expr>>,
) -> Result<Option<Box<Expr>>> {
    Ok(opt.map(|n| fold.fold_expr(*n)).transpose()?.map(Box::from))
}

pub fn fold_interpolate_item<F: ?Sized + AstFold>(
    fold: &mut F,
    interpolate_item: InterpolateItem,
) -> Result<InterpolateItem> {
    Ok(match interpolate_item {
        InterpolateItem::String(string) => InterpolateItem::String(string),
        InterpolateItem::Expr(expr) => InterpolateItem::Expr(Box::new(fold.fold_expr(*expr)?)),
    })
}

fn fold_cases<F: ?Sized + AstFold>(
    fold: &mut F,
    cases: Vec<SwitchCase>,
) -> Result<Vec<SwitchCase>> {
    cases
        .into_iter()
        .map(|c| fold_switch_case(fold, c))
        .try_collect()
}

pub fn fold_switch_case<F: ?Sized + AstFold>(fold: &mut F, case: SwitchCase) -> Result<SwitchCase> {
    Ok(SwitchCase {
        condition: fold.fold_expr(case.condition)?,
        value: fold.fold_expr(case.value)?,
    })
}

pub fn fold_column_sorts<F: ?Sized + AstFold>(
    fold: &mut F,
    sort: Vec<ColumnSort>,
) -> Result<Vec<ColumnSort>> {
    sort.into_iter()
        .map(|s| fold_column_sort(fold, s))
        .try_collect()
}

pub fn fold_column_sort<T: ?Sized + AstFold>(
    fold: &mut T,
    sort_column: ColumnSort,
) -> Result<ColumnSort> {
    Ok(ColumnSort {
        direction: sort_column.direction,
        column: fold.fold_expr(sort_column.column)?,
    })
}

pub fn fold_func_call<T: ?Sized + AstFold>(fold: &mut T, func_call: FuncCall) -> Result<FuncCall> {
    Ok(FuncCall {
        name: Box::new(fold.fold_expr(*func_call.name)?),
        args: fold.fold_exprs(func_call.args)?,
        named_args: func_call
            .named_args
            .into_iter()
            .map(|(name, expr)| fold.fold_expr(expr).map(|e| (name, e)))
            .try_collect()?,
    })
}

pub fn fold_transform_call<T: ?Sized + AstFold>(
    fold: &mut T,
    t: TransformCall,
) -> Result<TransformCall> {
    Ok(TransformCall {
        kind: Box::new(fold_transform_kind(fold, *t.kind)?),
        input: Box::new(fold.fold_expr(*t.input)?),
        partition: fold.fold_exprs(t.partition)?,
        frame: fold.fold_window(t.frame)?,
        sort: fold_column_sorts(fold, t.sort)?,
    })
}

pub fn fold_transform_kind<T: ?Sized + AstFold>(
    fold: &mut T,
    t: TransformKind,
) -> Result<TransformKind> {
    use TransformKind::*;
    Ok(match t {
        Derive { assigns } => Derive {
            assigns: fold.fold_exprs(assigns)?,
        },
        Select { assigns } => Select {
            assigns: fold.fold_exprs(assigns)?,
        },
        Filter { filter } => Filter {
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Aggregate { assigns } => Aggregate {
            assigns: fold.fold_exprs(assigns)?,
        },
        Sort { by } => Sort {
            by: by
                .into_iter()
                .map(|s| fold_column_sort(fold, s))
                .try_collect()?,
        },
        Take { range } => Take {
            range: fold_range(fold, range)?,
        },
        Join { side, with, filter } => Join {
            side,
            with: Box::new(fold.fold_expr(*with)?),
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Concat(bottom) => Concat(Box::new(fold.fold_expr(*bottom)?)),
        Group { by, pipeline } => Group {
            by: fold.fold_exprs(by)?,
            pipeline: Box::new(fold.fold_expr(*pipeline)?),
        },
        Window {
            kind,
            range,
            pipeline,
        } => Window {
            kind,
            range: fold_range(fold, range)?,
            pipeline: Box::new(fold.fold_expr(*pipeline)?),
        },
    })
}

pub fn fold_closure<T: ?Sized + AstFold>(fold: &mut T, closure: Closure) -> Result<Closure> {
    Ok(Closure {
        body: Box::new(fold.fold_expr(*closure.body)?),
        args: closure
            .args
            .into_iter()
            .map(|item| fold.fold_expr(item))
            .try_collect()?,
        ..closure
    })
}

pub fn fold_func_def<T: ?Sized + AstFold>(fold: &mut T, func_def: FuncDef) -> Result<FuncDef> {
    Ok(FuncDef {
        name: func_def.name,
        positional_params: fold_func_param(fold, func_def.positional_params)?,
        named_params: fold_func_param(fold, func_def.named_params)?,
        body: Box::new(fold.fold_expr(*func_def.body)?),
        return_ty: func_def.return_ty,
    })
}

pub fn fold_func_param<T: ?Sized + AstFold>(
    fold: &mut T,
    nodes: Vec<FuncParam>,
) -> Result<Vec<FuncParam>> {
    nodes
        .into_iter()
        .map(|param| {
            Ok(FuncParam {
                default_value: param.default_value.map(|n| fold.fold_expr(n)).transpose()?,
                ..param
            })
        })
        .try_collect()
}

pub fn fold_type<T: ?Sized + AstFold>(fold: &mut T, t: Ty) -> Result<Ty> {
    Ok(match t {
        Ty::Literal(_) => t,
        Ty::Parameterized(t, p) => {
            Ty::Parameterized(Box::new(fold.fold_type(*t)?), Box::new(fold.fold_type(*p)?))
        }
        Ty::AnyOf(ts) => Ty::AnyOf(ts.into_iter().map(|t| fold_type(fold, t)).try_collect()?),
        _ => t,
    })
}
