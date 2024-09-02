use std::collections::HashMap;

use crate::ir::decl::{DeclKind, RootModule};
use crate::ir::pl::*;
use crate::semantic::NS_LOCAL;
use crate::{Error, Result, WithErrorInfo};

pub struct Inliner<'a> {
    root_mod: &'a RootModule,
}

impl<'a> Inliner<'a> {
    pub fn run(root_mod: &'a RootModule, expr: Expr) -> Expr {
        let mut i = Inliner { root_mod };
        i.fold_expr(expr).unwrap()
    }

    fn lookup_expr(&self, fq_ident: &Ident) -> Option<&Expr> {
        let mut ident = fq_ident;
        loop {
            let decl = self.root_mod.module.get(ident)?;

            match &decl.kind {
                DeclKind::Expr(expr) => {
                    if let ExprKind::Ident(i) = &expr.kind {
                        ident = i;
                    } else {
                        break Some(expr);
                    }
                }
                DeclKind::Import(i) => ident = i,
                _ => break None,
            }
        }
    }

    fn lookup_func(&self, ident: &Expr) -> Option<(Ident, &Func)> {
        let fq_ident = ident.kind.as_ident()?;
        let func_decl = self.lookup_expr(fq_ident)?;
        let func = func_decl.kind.as_func()?;
        Some((fq_ident.clone(), func))
    }
}

impl<'a> PlFold for Inliner<'a> {
    fn fold_expr(&mut self, mut expr: Expr) -> crate::Result<Expr> {
        expr.kind = match expr.kind {
            ExprKind::FuncApplication(FuncApplication { func, args }) => {
                if let Some((fn_ident, fn_func)) = self.lookup_func(&func) {
                    if let ExprKind::Internal(internal) = &fn_func.body.kind {
                        // rq operator
                        ExprKind::RqOperator {
                            name: internal.clone(),
                            args: self.fold_exprs(args)?,
                        }
                    } else {
                        // inline
                        FuncInliner::run(fn_ident, fn_func, args)?.kind
                    }
                } else {
                    // potentially throw an error here, since we don't know how to translate this
                    // function a relational expression?
                    // it is gonna error out in lowering so we might as well do it earlier
                    ExprKind::FuncApplication(FuncApplication { func, args })
                }
            }
            ExprKind::Ident(fq_ident) => {
                if let Some(expr) = self.lookup_expr(&fq_ident) {
                    match &expr.kind {
                        ExprKind::Internal(internal) => ExprKind::RqOperator {
                            name: internal.clone(),
                            args: vec![],
                        },
                        ExprKind::Param(key) if !expr.ty.as_ref().unwrap().is_relation() => {
                            ExprKind::Param(key.clone())
                        }
                        ExprKind::Literal(lit) => ExprKind::Literal(lit.clone()),
                        _ => ExprKind::Ident(fq_ident),
                    }
                } else {
                    ExprKind::Ident(fq_ident)
                }
            }
            k => fold_expr_kind(self, k)?,
        };
        Ok(expr)
    }
}

struct FuncInliner<'a> {
    // fq ident of the functions we are inlining
    fn_ident: Ident,

    param_args: HashMap<&'a str, Expr>,
}

impl<'a> FuncInliner<'a> {
    fn run(fn_ident: Ident, fn_func: &Func, args: Vec<Expr>) -> Result<Expr> {
        let mut i = FuncInliner {
            fn_ident,
            param_args: HashMap::with_capacity(fn_func.params.len()),
        };

        for (param, arg) in itertools::zip_eq(&fn_func.params, args) {
            i.param_args.insert(param.name.as_str(), arg);
        }
        i.fold_expr(*fn_func.body.clone())
    }
}

impl PlFold for FuncInliner<'_> {
    fn fold_expr(&mut self, mut expr: Expr) -> crate::Result<Expr> {
        expr.kind = match expr.kind {
            ExprKind::Ident(fq_ident) => {
                if fq_ident == self.fn_ident {
                    return Err(
                        Error::new_simple("recursive functions not supported").with_span(expr.span)
                    );
                }

                if fq_ident.starts_with_path(&[NS_LOCAL]) {
                    assert_eq!(fq_ident.len(), 2);
                    let param_name = fq_ident.name;

                    let param = self.param_args.get(param_name.as_str()).unwrap();
                    param.kind.clone()
                } else {
                    ExprKind::Ident(fq_ident)
                }
            }
            k => fold_expr_kind(self, k)?,
        };
        Ok(expr)
    }
}
