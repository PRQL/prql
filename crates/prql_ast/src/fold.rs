/// A trait to "fold" a PRQL AST (similar to a visitor), so we can transitively
/// apply some logic to a whole tree by just defining how we want to handle each
/// type.
use anyhow::Result;
use itertools::Itertools;

use crate::expr::{BinaryExpr, ExprKind, Pipeline, UnaryExpr, *};
use crate::stmt::*;

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

pub trait Fold<T: Extension, Folder>: Sized
// Fold is generic over the Folder so that it can require additional methods
// e.g. implementing Fold for ExprKindExtra also requires a fold_transform_call
// method on the folder.
where
    Folder: AstFold<T>,
    T::ExprKindVariant: Fold<T, Folder>,
    T::FuncExtra: Fold<T, Folder>,
{
    fn fold(self, folder: &mut Folder) -> Result<Self>;
}

pub trait AstFold<T: Extension>: Sized
// FUTURE: once trait aliases are stable define a FoldableExtension trait alias to get rid of the where clauses everywhere
where
    T::ExprKindVariant: Fold<T, Self>,
    T::FuncExtra: Fold<T, Self>,
{
    fn fold_stmt(&mut self, mut stmt: Stmt<T>) -> Result<Stmt<T>> {
        stmt.kind = fold_stmt_kind(self, stmt.kind)?;
        Ok(stmt)
    }
    fn fold_stmts(&mut self, stmts: Vec<Stmt<T>>) -> Result<Vec<Stmt<T>>> {
        stmts.into_iter().map(|stmt| self.fold_stmt(stmt)).collect()
    }
    fn fold_expr(&mut self, mut expr: Expr<T>) -> Result<Expr<T>> {
        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
    fn fold_expr_kind(&mut self, expr_kind: ExprKind<T>) -> Result<ExprKind<T>> {
        fold_expr_kind(self, expr_kind)
    }
    fn fold_exprs(&mut self, exprs: Vec<Expr<T>>) -> Result<Vec<Expr<T>>> {
        exprs.into_iter().map(|node| self.fold_expr(node)).collect()
    }
    fn fold_var_def(&mut self, var_def: VarDef<T>) -> Result<VarDef<T>> {
        fold_var_def(self, var_def)
    }
    fn fold_type_def(&mut self, ty_def: TypeDef<T>) -> Result<TypeDef<T>> {
        Ok(TypeDef {
            value: fold_optional_box(self, ty_def.value)?,
        })
    }
    fn fold_module_def(&mut self, module_def: ModuleDef<T>) -> Result<ModuleDef<T>> {
        fold_module_def(self, module_def)
    }
    fn fold_pipeline(&mut self, pipeline: Pipeline<T>) -> Result<Pipeline<T>> {
        fold_pipeline(self, pipeline)
    }
    fn fold_func_call(&mut self, func_call: FuncCall<T>) -> Result<FuncCall<T>> {
        fold_func_call(self, func_call)
    }
    fn fold_func(&mut self, func: Func<T>) -> Result<Func<T>> {
        fold_func(self, func)
    }
    fn fold_interpolate_item(
        &mut self,
        sstring_item: InterpolateItem<Expr<T>>,
    ) -> Result<InterpolateItem<Expr<T>>> {
        fold_interpolate_item(self, sstring_item)
    }

    fn fold_extra_expr_kind(&mut self, val: T::ExprKindVariant) -> Result<T::ExprKindVariant> {
        val.fold(self)
    }

    fn fold_extra_func(&mut self, val: T::FuncExtra) -> Result<T::FuncExtra> {
        val.fold(self)
    }
}

pub fn fold_expr_kind<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    expr_kind: ExprKind<T>,
) -> Result<ExprKind<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    use ExprKind::*;
    Ok(match expr_kind {
        Ident(ident) => Ident(ident),
        Binary(BinaryExpr { op, left, right }) => Binary(BinaryExpr {
            op,
            left: Box::new(fold.fold_expr(*left)?),
            right: Box::new(fold.fold_expr(*right)?),
        }),
        Unary(UnaryExpr { op, expr }) => Unary(UnaryExpr {
            op,
            expr: Box::new(fold.fold_expr(*expr)?),
        }),
        Tuple(items) => Tuple(fold.fold_exprs(items)?),
        Array(items) => Array(fold.fold_exprs(items)?),
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
        Case(cases) => Case(fold_cases(fold, cases)?),

        FuncCall(func_call) => FuncCall(fold.fold_func_call(func_call)?),
        Func(closure) => Func(Box::new(fold.fold_func(*closure)?)),

        Other(extra_kind) => Other(fold.fold_extra_expr_kind(extra_kind)?),

        // None of these capture variables, so we don't need to fold them.
        Param(_) | Internal(_) | Literal(_) => expr_kind,
    })
}

pub fn fold_stmt_kind<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    stmt_kind: StmtKind<T>,
) -> Result<StmtKind<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    use StmtKind::*;
    Ok(match stmt_kind {
        // FuncDef(func) => FuncDef(fold.fold_func_def(func)?),
        VarDef(var_def) => VarDef(fold.fold_var_def(var_def)?),
        TypeDef(type_def) => TypeDef(fold.fold_type_def(type_def)?),
        ModuleDef(module_def) => ModuleDef(fold.fold_module_def(module_def)?),
        QueryDef(_) => stmt_kind,
    })
}

fn fold_module_def<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    module_def: ModuleDef<T>,
) -> Result<ModuleDef<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(ModuleDef {
        stmts: fold.fold_stmts(module_def.stmts)?,
    })
}

pub fn fold_var_def<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    var_def: VarDef<T>,
) -> Result<VarDef<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(VarDef {
        value: Box::new(fold.fold_expr(*var_def.value)?),
        ty_expr: fold_optional_box(fold, var_def.ty_expr)?,
        kind: var_def.kind,
    })
}

pub fn fold_range<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    Range { start, end }: Range<Box<Expr<T>>>,
) -> Result<Range<Box<Expr<T>>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(Range {
        start: fold_optional_box(fold, start)?,
        end: fold_optional_box(fold, end)?,
    })
}

pub fn fold_pipeline<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    pipeline: Pipeline<T>,
) -> Result<Pipeline<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(Pipeline {
        exprs: fold.fold_exprs(pipeline.exprs)?,
    })
}

// This aren't strictly in the hierarchy, so we don't need to
// have an assoc. function for `fold_optional_box` — we just
// call out to the function in this module
pub fn fold_optional_box<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    opt: Option<Box<Expr<T>>>,
) -> Result<Option<Box<Expr<T>>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(opt.map(|n| fold.fold_expr(*n)).transpose()?.map(Box::from))
}

pub fn fold_interpolate_item<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    interpolate_item: InterpolateItem<Expr<T>>,
) -> Result<InterpolateItem<Expr<T>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(match interpolate_item {
        InterpolateItem::String(string) => InterpolateItem::String(string),
        InterpolateItem::Expr { expr, format } => InterpolateItem::Expr {
            expr: Box::new(fold.fold_expr(*expr)?),
            format,
        },
    })
}

fn fold_cases<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    cases: Vec<SwitchCase<Box<Expr<T>>>>,
) -> Result<Vec<SwitchCase<Box<Expr<T>>>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    cases
        .into_iter()
        .map(|c| fold_switch_case(fold, c))
        .try_collect()
}

pub fn fold_switch_case<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    case: SwitchCase<Box<Expr<T>>>,
) -> Result<SwitchCase<Box<Expr<T>>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(SwitchCase {
        condition: Box::new(fold.fold_expr(*case.condition)?),
        value: Box::new(fold.fold_expr(*case.value)?),
    })
}

pub fn fold_func_call<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    func_call: FuncCall<T>,
) -> Result<FuncCall<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
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

pub fn fold_func<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    func: Func<T>,
) -> Result<Func<T>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
    Ok(Func {
        body: Box::new(fold.fold_expr(*func.body)?),
        extra: fold.fold_extra_func(func.extra)?,
        ..func
    })
}

pub fn fold_func_param<T: Extension, F: ?Sized + AstFold<T>>(
    fold: &mut F,
    nodes: Vec<FuncParam<T>>,
) -> Result<Vec<FuncParam<T>>>
where
    T::ExprKindVariant: Fold<T, F>,
    T::FuncExtra: Fold<T, F>,
{
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
