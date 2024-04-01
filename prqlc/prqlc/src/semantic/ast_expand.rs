use std::collections::HashMap;

use itertools::Itertools;
use prqlc_ast::error::WithErrorInfo;

use crate::ast::*;
use crate::ir::decl;
use crate::ir::pl::{self, new_binop};
use crate::semantic::{NS_THAT, NS_THIS};
use crate::{Error, Result};

/// An AST pass that maps AST to PL.
pub fn expand_expr(expr: Expr) -> Result<pl::Expr> {
    let kind = match expr.kind {
        ExprKind::Ident(v) => pl::ExprKind::Ident(Ident::from_name(v)),
        ExprKind::Indirection { base, field } => {
            let field_as_name = match field {
                IndirectionKind::Name(n) => n,
                IndirectionKind::Position(_) => Err(Error::new_simple(
                    "Positional indirection not supported yet",
                )
                .with_span(expr.span))?,
                IndirectionKind::Star => "*".to_string(),
            };

            // convert indirections into ident
            // (in the future, resolve will support proper indirection handling)
            let base = expand_expr_box(base)?;
            let base_ident = base.kind.into_ident().map_err(|_| {
                Error::new_simple("Indirection (the dot) is supported only on names.")
                    .with_span(expr.span)
            })?;

            let ident = base_ident + Ident::from_name(field_as_name);
            pl::ExprKind::Ident(ident)
        }
        ExprKind::Literal(v) => pl::ExprKind::Literal(v),
        ExprKind::Pipeline(v) => {
            let mut e = desugar_pipeline(v)?;
            e.alias = expr.alias.or(e.alias);
            return Ok(e);
        }
        ExprKind::Tuple(v) => pl::ExprKind::Tuple(expand_exprs(v)?),
        ExprKind::Array(v) => pl::ExprKind::Array(expand_exprs(v)?),

        ExprKind::Range(v) => expands_range(v)?,

        ExprKind::Unary(unary) => expand_unary(unary)?,
        ExprKind::Binary(binary) => expand_binary(binary)?,

        ExprKind::FuncCall(v) => pl::ExprKind::FuncCall(pl::FuncCall {
            name: expand_expr_box(v.name)?,
            args: expand_exprs(v.args)?,
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| -> Result<_> { Ok((k, expand_expr(v)?)) })
                .try_collect()?,
        }),
        ExprKind::Func(v) => pl::ExprKind::Func(
            pl::Func {
                return_ty: v.return_ty,
                body: expand_expr_box(v.body)?,
                params: expand_func_params(v.params)?,
                named_params: expand_func_params(v.named_params)?,
                name_hint: None,
                args: Vec::new(),
                env: HashMap::new(),
                generic_type_params: v.generic_type_params,
            }
            .into(),
        ),
        ExprKind::SString(v) => pl::ExprKind::SString(
            v.into_iter()
                .map(|v| v.try_map(expand_expr))
                .try_collect()?,
        ),
        ExprKind::FString(v) => pl::ExprKind::FString(
            v.into_iter()
                .map(|v| v.try_map(expand_expr))
                .try_collect()?,
        ),
        ExprKind::Case(v) => pl::ExprKind::Case(
            v.into_iter()
                .map(|case| -> Result<_> {
                    Ok(pl::SwitchCase {
                        condition: expand_expr_box(case.condition)?,
                        value: expand_expr_box(case.value)?,
                    })
                })
                .try_collect()?,
        ),
        ExprKind::Param(v) => pl::ExprKind::Param(v),
        ExprKind::Internal(v) => pl::ExprKind::Internal(v),
    };

    Ok(pl::Expr {
        kind,
        span: expr.span,
        alias: expr.alias,
        id: None,
        target_id: None,
        ty: None,
        lineage: None,
        needs_window: false,
        flatten: false,
    })
}

/// De-sugars range `a..b` into `{start=a, end=b}`. Open bounds are mapped into `null`.
fn expands_range(v: generic::Range<Box<Expr>>) -> Result<pl::ExprKind> {
    let mut start = v
        .start
        .map(|e| expand_expr(*e))
        .transpose()?
        .unwrap_or_else(|| pl::Expr::new(Literal::Null));
    start.alias = Some("start".into());
    let mut end = v
        .end
        .map(|e| expand_expr(*e))
        .transpose()?
        .unwrap_or_else(|| pl::Expr::new(Literal::Null));
    end.alias = Some("end".into());
    Ok(pl::ExprKind::Tuple(vec![start, end]))
}

fn expand_exprs(exprs: Vec<Expr>) -> Result<Vec<pl::Expr>> {
    exprs.into_iter().map(expand_expr).collect()
}

#[allow(clippy::boxed_local)]
fn expand_expr_box(expr: Box<Expr>) -> Result<Box<pl::Expr>> {
    Ok(Box::new(expand_expr(*expr)?))
}

fn desugar_pipeline(mut pipeline: Pipeline) -> Result<pl::Expr> {
    let value = pipeline.exprs.remove(0);
    let mut value = expand_expr(value)?;

    for expr in pipeline.exprs {
        let expr = expand_expr(expr)?;
        let span = expr.span;

        value = pl::Expr::new(pl::ExprKind::FuncCall(pl::FuncCall::new_simple(
            expr,
            vec![value],
        )));
        value.span = span;
    }

    Ok(value)
}

/// Desugar unary operators into function calls.
fn expand_unary(UnaryExpr { op, expr }: UnaryExpr) -> Result<pl::ExprKind> {
    use UnOp::*;

    let expr = expand_expr(*expr)?;

    let func_name = match op {
        Neg => ["std", "neg"],
        Not => ["std", "not"],
        Add => return Ok(expr.kind),
        EqSelf => {
            let pl::ExprKind::Ident(ident) = expr.kind else {
                return Err(Error::new_simple(
                    "you can only use column names with self-equality operator",
                ));
            };
            if !ident.path.is_empty() {
                return Err(Error::new_simple(
                    "you cannot use namespace prefix with self-equality operator",
                ));
            }

            let left = pl::Expr {
                span: expr.span,
                ..pl::Expr::new(Ident {
                    path: vec![NS_THIS.to_string()],
                    name: ident.name.clone(),
                })
            };
            let right = pl::Expr {
                span: expr.span,
                ..pl::Expr::new(Ident {
                    path: vec![NS_THAT.to_string()],
                    name: ident.name,
                })
            };
            return Ok(new_binop(left, &["std", "eq"], right).kind);
        }
    };
    Ok(pl::ExprKind::FuncCall(pl::FuncCall::new_simple(
        pl::Expr::new(Ident::from_path(func_name.to_vec())),
        vec![expr],
    )))
}

/// Desugar binary operators into function calls.
fn expand_binary(BinaryExpr { op, left, right }: BinaryExpr) -> Result<pl::ExprKind> {
    let left = expand_expr(*left)?;
    let right = expand_expr(*right)?;

    let func_name = match op {
        BinOp::Mul => ["std", "mul"],
        BinOp::DivInt => ["std", "div_i"],
        BinOp::DivFloat => ["std", "div_f"],
        BinOp::Mod => ["std", "mod"],
        BinOp::Pow => ["std", "pow"],
        BinOp::Add => ["std", "add"],
        BinOp::Sub => ["std", "sub"],
        BinOp::Eq => ["std", "eq"],
        BinOp::Ne => ["std", "ne"],
        BinOp::Gt => ["std", "gt"],
        BinOp::Lt => ["std", "lt"],
        BinOp::Gte => ["std", "gte"],
        BinOp::Lte => ["std", "lte"],
        BinOp::RegexSearch => ["std", "regex_search"],
        BinOp::And => ["std", "and"],
        BinOp::Or => ["std", "or"],
        BinOp::Coalesce => ["std", "coalesce"],
    };
    Ok(new_binop(left, &func_name, right).kind)
}

fn expand_func_param(value: FuncParam) -> Result<pl::FuncParam> {
    Ok(pl::FuncParam {
        name: value.name,
        ty: value.ty,
        default_value: value.default_value.map(expand_expr_box).transpose()?,
    })
}

fn expand_func_params(value: Vec<FuncParam>) -> Result<Vec<pl::FuncParam>> {
    value.into_iter().map(expand_func_param).collect()
}

fn expand_stmt(value: Stmt) -> Result<pl::Stmt> {
    Ok(pl::Stmt {
        id: None,
        kind: expand_stmt_kind(value.kind)?,
        span: value.span,
        annotations: value
            .annotations
            .into_iter()
            .map(expand_annotation)
            .try_collect()?,
    })
}

pub fn expand_module_def(v: ModuleDef) -> Result<pl::ModuleDef> {
    Ok(pl::ModuleDef {
        name: v.name,
        stmts: expand_stmts(v.stmts)?,
    })
}

fn expand_stmts(value: Vec<Stmt>) -> Result<Vec<pl::Stmt>> {
    value.into_iter().map(expand_stmt).collect()
}

fn expand_stmt_kind(value: StmtKind) -> Result<pl::StmtKind> {
    Ok(match value {
        StmtKind::QueryDef(v) => pl::StmtKind::QueryDef(v),
        StmtKind::VarDef(v) => pl::StmtKind::VarDef(pl::VarDef {
            name: v.name,
            value: v.value.map(expand_expr_box).transpose()?,
            ty: v.ty,
        }),
        StmtKind::TypeDef(v) => pl::StmtKind::TypeDef(pl::TypeDef {
            name: v.name,
            value: v.value,
        }),
        StmtKind::ModuleDef(v) => pl::StmtKind::ModuleDef(expand_module_def(v)?),
        StmtKind::ImportDef(v) => pl::StmtKind::ImportDef(pl::ImportDef {
            alias: v.alias,
            name: v.name,
        }),
    })
}

fn expand_annotation(value: Annotation) -> Result<pl::Annotation> {
    Ok(pl::Annotation {
        expr: expand_expr_box(value.expr)?,
    })
}

/// An AST pass that tries to revert the mapping from AST to PL
pub fn restrict_expr(expr: pl::Expr) -> Expr {
    Expr {
        kind: restrict_expr_kind(expr.kind),
        span: expr.span,
        alias: expr.alias,
    }
}

#[allow(clippy::boxed_local)]
fn restrict_expr_box(expr: Box<pl::Expr>) -> Box<Expr> {
    Box::new(restrict_expr(*expr))
}

fn restrict_exprs(exprs: Vec<pl::Expr>) -> Vec<Expr> {
    exprs.into_iter().map(restrict_expr).collect()
}

fn restrict_expr_kind(value: pl::ExprKind) -> ExprKind {
    match value {
        pl::ExprKind::Ident(v) => {
            let mut parts = v.into_iter();
            let mut base = Box::new(Expr::new(ExprKind::Ident(parts.next().unwrap())));
            for part in parts {
                let field = IndirectionKind::Name(part);
                base = Box::new(Expr::new(ExprKind::Indirection { base, field }))
            }
            base.kind
        }
        pl::ExprKind::Literal(v) => ExprKind::Literal(v),
        pl::ExprKind::Tuple(v) => ExprKind::Tuple(restrict_exprs(v)),
        pl::ExprKind::Array(v) => ExprKind::Array(restrict_exprs(v)),
        pl::ExprKind::FuncCall(v) => ExprKind::FuncCall(FuncCall {
            name: restrict_expr_box(v.name),
            args: restrict_exprs(v.args),
            named_args: v
                .named_args
                .into_iter()
                .map(|(k, v)| (k, restrict_expr(v)))
                .collect(),
        }),
        pl::ExprKind::Func(v) => {
            let func = ExprKind::Func(
                Func {
                    return_ty: v.return_ty,
                    body: restrict_expr_box(v.body),
                    params: restrict_func_params(v.params),
                    named_params: restrict_func_params(v.named_params),
                    generic_type_params: v.generic_type_params,
                }
                .into(),
            );
            if v.args.is_empty() {
                func
            } else {
                ExprKind::FuncCall(FuncCall {
                    name: Box::new(Expr::new(func)),
                    args: restrict_exprs(v.args),
                    named_args: Default::default(),
                })
            }
        }
        pl::ExprKind::SString(v) => {
            ExprKind::SString(v.into_iter().map(|v| v.map(restrict_expr)).collect())
        }
        pl::ExprKind::FString(v) => {
            ExprKind::FString(v.into_iter().map(|v| v.map(restrict_expr)).collect())
        }
        pl::ExprKind::Case(v) => ExprKind::Case(
            v.into_iter()
                .map(|case| SwitchCase {
                    condition: restrict_expr_box(case.condition),
                    value: restrict_expr_box(case.value),
                })
                .collect(),
        ),
        pl::ExprKind::Param(v) => ExprKind::Param(v),
        pl::ExprKind::Internal(v) => ExprKind::Internal(v),

        // TODO: these are not correct, they are producing invalid PRQL
        pl::ExprKind::All { within, .. } => restrict_expr(*within).kind,
        pl::ExprKind::TransformCall(tc) => {
            ExprKind::Ident(format!("({} ...)", tc.kind.as_ref().as_ref()))
        }
        pl::ExprKind::RqOperator { name, .. } => ExprKind::Ident(format!("({} ...)", name)),
    }
}

fn restrict_func_params(value: Vec<pl::FuncParam>) -> Vec<FuncParam> {
    value.into_iter().map(restrict_func_param).collect()
}

fn restrict_func_param(value: pl::FuncParam) -> FuncParam {
    FuncParam {
        name: value.name,
        ty: value.ty,
        default_value: value.default_value.map(restrict_expr_box),
    }
}

/// Restricts a tuple of form `{start=a, end=b}` into a range `a..b`.
pub fn try_restrict_range(expr: pl::Expr) -> Result<(pl::Expr, pl::Expr), pl::Expr> {
    let pl::ExprKind::Tuple(fields) = expr.kind else {
        return Err(expr);
    };

    if fields.len() != 2
        || fields[0].alias.as_deref() != Some("start")
        || fields[1].alias.as_deref() != Some("end")
    {
        return Err(pl::Expr {
            kind: pl::ExprKind::Tuple(fields),
            ..expr
        });
    }

    let [start, end]: [pl::Expr; 2] = fields.try_into().unwrap();

    Ok((start, end))
}

/// Returns None if the Expr is a null literal and Some(expr) otherwise.
pub fn restrict_null_literal(expr: pl::Expr) -> Option<pl::Expr> {
    if let pl::ExprKind::Literal(Literal::Null) = expr.kind {
        None
    } else {
        Some(expr)
    }
}

pub fn restrict_module_def(def: pl::ModuleDef) -> ModuleDef {
    ModuleDef {
        name: def.name,
        stmts: restrict_stmts(def.stmts),
    }
}

fn restrict_stmts(stmts: Vec<pl::Stmt>) -> Vec<Stmt> {
    stmts.into_iter().map(restrict_stmt).collect()
}

fn restrict_stmt(stmt: pl::Stmt) -> Stmt {
    let kind = match stmt.kind {
        pl::StmtKind::QueryDef(def) => StmtKind::QueryDef(def),
        pl::StmtKind::VarDef(def) => StmtKind::VarDef(VarDef {
            kind: VarDefKind::Let,
            name: def.name,
            value: def.value.map(restrict_expr_box),
            ty: def.ty,
        }),
        pl::StmtKind::TypeDef(def) => StmtKind::TypeDef(TypeDef {
            name: def.name,
            value: def.value,
        }),
        pl::StmtKind::ModuleDef(def) => StmtKind::ModuleDef(restrict_module_def(def)),
        pl::StmtKind::ImportDef(def) => StmtKind::ImportDef(ImportDef {
            name: def.name,
            alias: def.alias,
        }),
    };

    Stmt {
        kind,
        span: stmt.span,
        annotations: stmt
            .annotations
            .into_iter()
            .map(restrict_annotation)
            .collect(),
    }
}

pub fn restrict_annotation(value: pl::Annotation) -> Annotation {
    Annotation {
        expr: restrict_expr_box(value.expr),
    }
}

pub fn restrict_module(value: decl::Module) -> ModuleDef {
    let mut stmts = Vec::new();
    for (name, decl) in value.names.into_iter().sorted_by_key(|x| x.0.clone()) {
        stmts.extend(restrict_decl(name, decl))
    }

    ModuleDef {
        name: "".to_string(),
        stmts,
    }
}

fn restrict_decl(name: String, value: decl::Decl) -> Option<Stmt> {
    let kind = match value.kind {
        decl::DeclKind::Module(module) => StmtKind::ModuleDef(ModuleDef {
            name,
            stmts: restrict_module(module).stmts,
        }),
        decl::DeclKind::LayeredModules(mut stack) => {
            let module = stack.pop()?;

            StmtKind::ModuleDef(ModuleDef {
                name,
                stmts: restrict_module(module).stmts,
            })
        }
        decl::DeclKind::TableDecl(table_decl) => StmtKind::VarDef(VarDef {
            kind: VarDefKind::Let,
            name: name.clone(),
            value: Some(Box::new(match table_decl.expr {
                decl::TableExpr::RelationVar(expr) => restrict_expr(*expr),
                decl::TableExpr::LocalTable => Expr::new(ExprKind::Internal("local_table".into())),
                decl::TableExpr::None => {
                    Expr::new(ExprKind::Internal("literal_tracker".to_string()))
                }
                decl::TableExpr::Param(id) => Expr::new(ExprKind::Param(id)),
            })),
            ty: table_decl.ty,
        }),

        decl::DeclKind::InstanceOf(ident, _) => {
            new_internal_stmt(name, format!("instance_of.{ident}"))
        }
        decl::DeclKind::Column(id) => new_internal_stmt(name, format!("column.{id}")),
        decl::DeclKind::Infer(_) => new_internal_stmt(name, "infer".to_string()),

        decl::DeclKind::Expr(mut expr) => StmtKind::VarDef(VarDef {
            kind: VarDefKind::Let,
            name,
            ty: expr.ty.take(),
            value: Some(restrict_expr_box(expr)),
        }),
        decl::DeclKind::Ty(ty) => StmtKind::TypeDef(TypeDef {
            name,
            value: Some(ty),
        }),
        decl::DeclKind::QueryDef(query_def) => StmtKind::QueryDef(Box::new(query_def)),
        decl::DeclKind::Import(ident) => StmtKind::ImportDef(ImportDef {
            alias: Some(name),
            name: ident,
        }),
    };
    Some(Stmt::new(kind))
}

fn new_internal_stmt(name: String, internal: String) -> StmtKind {
    StmtKind::VarDef(VarDef {
        kind: VarDefKind::Let,
        name,
        value: Some(Box::new(Expr::new(ExprKind::Internal(internal)))),
        ty: None,
    })
}
