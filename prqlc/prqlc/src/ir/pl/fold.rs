/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;
use prqlc_ast::{TupleField, Ty, TyFunc, TyKind};

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
pub trait PlFold {
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
    fn fold_var_def(&mut self, var_def: VarDef) -> Result<VarDef> {
        fold_var_def(self, var_def)
    }
    fn fold_type_def(&mut self, ty_def: TypeDef) -> Result<TypeDef> {
        Ok(TypeDef {
            name: ty_def.name,
            value: ty_def.value.map(|x| self.fold_type(x)).transpose()?,
        })
    }
    fn fold_module_def(&mut self, module_def: ModuleDef) -> Result<ModuleDef> {
        fold_module_def(self, module_def)
    }
    fn fold_func_call(&mut self, func_call: FuncCall) -> Result<FuncCall> {
        fold_func_call(self, func_call)
    }
    fn fold_transform_call(&mut self, transform_call: TransformCall) -> Result<TransformCall> {
        fold_transform_call(self, transform_call)
    }
    fn fold_func(&mut self, func: Func) -> Result<Func> {
        fold_func(self, func)
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

pub fn fold_expr_kind<T: ?Sized + PlFold>(fold: &mut T, expr_kind: ExprKind) -> Result<ExprKind> {
    use ExprKind::*;
    Ok(match expr_kind {
        Ident(ident) => Ident(ident),
        All { within, except } => All {
            within: Box::new(fold.fold_expr(*within)?),
            except: Box::new(fold.fold_expr(*except)?),
        },
        Tuple(items) => Tuple(fold.fold_exprs(items)?),
        Array(items) => Array(fold.fold_exprs(items)?),
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
        Case(cases) => Case(fold_cases(fold, cases)?),

        FuncCall(func_call) => FuncCall(fold.fold_func_call(func_call)?),
        Func(closure) => Func(Box::new(fold.fold_func(*closure)?)),

        TransformCall(transform) => TransformCall(fold.fold_transform_call(transform)?),
        RqOperator { name, args } => RqOperator {
            name,
            args: fold.fold_exprs(args)?,
        },

        // None of these capture variables, so we don't need to fold them.
        Param(_) | Internal(_) | Literal(_) => expr_kind,
    })
}

pub fn fold_stmt_kind<T: ?Sized + PlFold>(fold: &mut T, stmt_kind: StmtKind) -> Result<StmtKind> {
    use StmtKind::*;
    Ok(match stmt_kind {
        // FuncDef(func) => FuncDef(fold.fold_func_def(func)?),
        VarDef(var_def) => VarDef(fold.fold_var_def(var_def)?),
        TypeDef(type_def) => TypeDef(fold.fold_type_def(type_def)?),
        ModuleDef(module_def) => ModuleDef(fold.fold_module_def(module_def)?),
        QueryDef(_) => stmt_kind,
    })
}

fn fold_module_def<F: ?Sized + PlFold>(fold: &mut F, module_def: ModuleDef) -> Result<ModuleDef> {
    Ok(ModuleDef {
        name: module_def.name,
        stmts: fold.fold_stmts(module_def.stmts)?,
    })
}

pub fn fold_var_def<F: ?Sized + PlFold>(fold: &mut F, var_def: VarDef) -> Result<VarDef> {
    Ok(VarDef {
        name: var_def.name,
        value: Box::new(fold.fold_expr(*var_def.value)?),
        ty: var_def.ty.map(|x| fold.fold_type(x)).transpose()?,
    })
}

pub fn fold_window<F: ?Sized + PlFold>(fold: &mut F, window: WindowFrame) -> Result<WindowFrame> {
    Ok(WindowFrame {
        kind: window.kind,
        range: fold_range(fold, window.range)?,
    })
}

pub fn fold_range<F: ?Sized + PlFold>(fold: &mut F, Range { start, end }: Range) -> Result<Range> {
    Ok(Range {
        start: fold_optional_box(fold, start)?,
        end: fold_optional_box(fold, end)?,
    })
}

// This aren't strictly in the hierarchy, so we don't need to
// have an assoc. function for `fold_optional_box` — we just
// call out to the function in this module
pub fn fold_optional_box<F: ?Sized + PlFold>(
    fold: &mut F,
    opt: Option<Box<Expr>>,
) -> Result<Option<Box<Expr>>> {
    Ok(opt.map(|n| fold.fold_expr(*n)).transpose()?.map(Box::from))
}

pub fn fold_interpolate_item<F: ?Sized + PlFold>(
    fold: &mut F,
    interpolate_item: InterpolateItem,
) -> Result<InterpolateItem> {
    Ok(match interpolate_item {
        InterpolateItem::String(string) => InterpolateItem::String(string),
        InterpolateItem::Expr { expr, format } => InterpolateItem::Expr {
            expr: Box::new(fold.fold_expr(*expr)?),
            format,
        },
    })
}

fn fold_cases<F: ?Sized + PlFold>(fold: &mut F, cases: Vec<SwitchCase>) -> Result<Vec<SwitchCase>> {
    cases
        .into_iter()
        .map(|c| fold_switch_case(fold, c))
        .try_collect()
}

pub fn fold_switch_case<F: ?Sized + PlFold>(fold: &mut F, case: SwitchCase) -> Result<SwitchCase> {
    Ok(SwitchCase {
        condition: Box::new(fold.fold_expr(*case.condition)?),
        value: Box::new(fold.fold_expr(*case.value)?),
    })
}

pub fn fold_column_sorts<F: ?Sized + PlFold>(
    fold: &mut F,
    sort: Vec<ColumnSort>,
) -> Result<Vec<ColumnSort>> {
    sort.into_iter()
        .map(|s| fold_column_sort(fold, s))
        .try_collect()
}

pub fn fold_column_sort<T: ?Sized + PlFold>(
    fold: &mut T,
    sort_column: ColumnSort,
) -> Result<ColumnSort> {
    Ok(ColumnSort {
        direction: sort_column.direction,
        column: Box::new(fold.fold_expr(*sort_column.column)?),
    })
}

pub fn fold_func_call<T: ?Sized + PlFold>(fold: &mut T, func_call: FuncCall) -> Result<FuncCall> {
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

pub fn fold_transform_call<T: ?Sized + PlFold>(
    fold: &mut T,
    t: TransformCall,
) -> Result<TransformCall> {
    Ok(TransformCall {
        kind: Box::new(fold_transform_kind(fold, *t.kind)?),
        input: Box::new(fold.fold_expr(*t.input)?),
        partition: fold_optional_box(fold, t.partition)?,
        frame: fold.fold_window(t.frame)?,
        sort: fold_column_sorts(fold, t.sort)?,
    })
}

pub fn fold_transform_kind<T: ?Sized + PlFold>(
    fold: &mut T,
    t: TransformKind,
) -> Result<TransformKind> {
    use TransformKind::*;
    Ok(match t {
        Derive { assigns } => Derive {
            assigns: Box::new(fold.fold_expr(*assigns)?),
        },
        Select { assigns } => Select {
            assigns: Box::new(fold.fold_expr(*assigns)?),
        },
        Filter { filter } => Filter {
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Aggregate { assigns } => Aggregate {
            assigns: Box::new(fold.fold_expr(*assigns)?),
        },
        Sort { by } => Sort {
            by: fold_column_sorts(fold, by)?,
        },
        Take { range } => Take {
            range: fold_range(fold, range)?,
        },
        Join { side, with, filter } => Join {
            side,
            with: Box::new(fold.fold_expr(*with)?),
            filter: Box::new(fold.fold_expr(*filter)?),
        },
        Append(bottom) => Append(Box::new(fold.fold_expr(*bottom)?)),
        Group { by, pipeline } => Group {
            by: Box::new(fold.fold_expr(*by)?),
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
        Loop(pipeline) => Loop(Box::new(fold.fold_expr(*pipeline)?)),
    })
}

pub fn fold_func<T: ?Sized + PlFold>(fold: &mut T, func: Func) -> Result<Func> {
    Ok(Func {
        body: Box::new(fold.fold_expr(*func.body)?),
        args: func
            .args
            .into_iter()
            .map(|item| fold.fold_expr(item))
            .try_collect()?,
        ..func
    })
}

pub fn fold_func_param<T: ?Sized + PlFold>(
    fold: &mut T,
    nodes: Vec<FuncParam>,
) -> Result<Vec<FuncParam>> {
    nodes
        .into_iter()
        .map(|param| {
            Ok(FuncParam {
                default_value: fold_optional_box(fold, param.default_value)?,
                ..param
            })
        })
        .try_collect()
}

#[inline]
pub fn fold_type_opt<T: ?Sized + PlFold>(fold: &mut T, ty: Option<Ty>) -> Result<Option<Ty>> {
    ty.map(|t| fold.fold_type(t)).transpose()
}

pub fn fold_type<T: ?Sized + PlFold>(fold: &mut T, ty: Ty) -> Result<Ty> {
    Ok(Ty {
        kind: match ty.kind {
            TyKind::Union(variants) => TyKind::Union(
                variants
                    .into_iter()
                    .map(|(name, ty)| -> Result<_> { Ok((name, fold.fold_type(ty)?)) })
                    .try_collect()?,
            ),
            TyKind::Tuple(fields) => TyKind::Tuple(
                fields
                    .into_iter()
                    .map(|field| -> Result<_> {
                        Ok(match field {
                            TupleField::Single(name, ty) => {
                                TupleField::Single(name, fold_type_opt(fold, ty)?)
                            }
                            TupleField::Wildcard(ty) => {
                                TupleField::Wildcard(fold_type_opt(fold, ty)?)
                            }
                        })
                    })
                    .try_collect()?,
            ),
            TyKind::Array(ty) => TyKind::Array(Box::new(fold.fold_type(*ty)?)),
            TyKind::Function(func) => TyKind::Function(
                func.map(|f| -> Result<_> {
                    Ok(TyFunc {
                        args: f
                            .args
                            .into_iter()
                            .map(|a| fold_type_opt(fold, a))
                            .try_collect()?,
                        return_ty: Box::new(fold_type_opt(fold, *f.return_ty)?),
                        name_hint: f.name_hint,
                    })
                })
                .transpose()?,
            ),
            TyKind::Difference { base, exclude } => TyKind::Difference {
                base: Box::new(fold.fold_type(*base)?),
                exclude: Box::new(fold.fold_type(*exclude)?),
            },
            TyKind::Any | TyKind::Ident(_) | TyKind::Primitive(_) | TyKind::Singleton(_) => ty.kind,
        },
        span: ty.span,
        name: ty.name,
    })
}
