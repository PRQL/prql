use ast::Span;
use itertools::Itertools;

use crate::ir::decl;
use crate::ir::pl::{self, ImportDef, PlFold};
use crate::{ast, utils, Error};
use crate::{Result, WithErrorInfo};

/// Runs name resolution for global names - names that refer to declarations.
///
/// Keeps track of all inter-declaration references.
/// Returns a resolution order.
pub fn resolve_decl_refs(root: &mut decl::RootModule) -> Result<Vec<pl::Ident>> {
    // resolve inter-declaration references
    let refs = {
        let mut r = ModuleRefResolver {
            root,
            refs: Default::default(),
            current_path: Vec::new(),
        };
        r.resolve_refs()?;
        r.refs
    };

    // HACK: put std.* declarations first
    // this is needed because during compilation of transforms, we inject refs to "std.lte" and a few others
    // sorting here makes std decls appear first in the final ordering
    let mut refs = refs;
    refs.sort_by_key(|(a, _)| !a.path.first().map_or(false, |p| p == "std"));

    // toposort the declarations
    // TODO: we might not need to compile all declarations if they are not used
    //   to prevent that, this start should be something else than None
    //   a list of all public declarations?
    // let main = pl::Ident::from_name("main");
    let order = utils::toposort::<ast::Ident>(&refs, None);

    if let Some(order) = order {
        Ok(order.into_iter().cloned().collect_vec())
    } else {
        todo!("error for a cyclic references between expressions")
    }
}

struct ModuleRefResolver<'a> {
    root: &'a mut decl::RootModule,
    current_path: Vec<String>,

    // TODO: maybe make these ids, instead of Ident?
    refs: Vec<(ast::Ident, Vec<ast::Ident>)>,
}

impl ModuleRefResolver<'_> {
    fn resolve_refs(&mut self) -> Result<()> {
        let path = &mut self.current_path;
        let module = self.root.module.get_submodule_mut(path).unwrap();

        let mut submodules = Vec::new();
        let mut unresolved_decls = Vec::new();
        for (name, decl) in &module.names {
            match &decl.kind {
                decl::DeclKind::Module(_) => {
                    submodules.push(name.clone());
                }
                decl::DeclKind::Unresolved(_) => {
                    unresolved_decls.push(name.clone());
                }
                _ => {}
            }
        }

        for name in unresolved_decls {
            // take the decl out of the module tree
            let mut decl = {
                let submodule = self.root.module.get_submodule_mut(path).unwrap();
                submodule.names.remove(&name).unwrap()
            };
            let span = decl
                .declared_at
                .and_then(|x| self.root.span_map.get(&x))
                .cloned();

            // resolve the decl
            path.push(name);
            let mut r = DeclRefResolver {
                root: self.root,
                decl_module_path: &path[0..(path.len() - 1)],
                refs: Vec::new(),
            };

            let stmt = decl.kind.into_unresolved().unwrap();
            let stmt = r.fold_stmt_kind(stmt, span)?;
            decl.kind = decl::DeclKind::Unresolved(stmt);

            let decl_ident = pl::Ident::from_path(path.clone());
            self.refs.push((decl_ident, r.refs));

            let name = path.pop().unwrap();

            // put the decl back in
            {
                let submodule = self.root.module.get_submodule_mut(path).unwrap();
                submodule.names.insert(name, decl);
            };
        }

        for name in submodules {
            self.current_path.push(name);
            self.resolve_refs()?;
            self.current_path.pop();
        }
        Ok(())
    }
}

struct DeclRefResolver<'a> {
    root: &'a decl::RootModule,
    decl_module_path: &'a [String],
    refs: Vec<pl::Ident>,
}

impl DeclRefResolver<'_> {
    fn fold_stmt_kind(&mut self, stmt: pl::StmtKind, span: Option<Span>) -> Result<pl::StmtKind> {
        Ok(match stmt {
            pl::StmtKind::QueryDef(_) => stmt,
            pl::StmtKind::VarDef(var_def) => pl::StmtKind::VarDef(self.fold_var_def(var_def)?),
            pl::StmtKind::TypeDef(ty_def) => pl::StmtKind::TypeDef(self.fold_type_def(ty_def)?),
            pl::StmtKind::ImportDef(import_def) => {
                pl::StmtKind::ImportDef(self.fold_import_def(import_def).with_span(span)?)
            }
            pl::StmtKind::ModuleDef(_) => unreachable!(),
        })
    }

    fn fold_import_def(&mut self, import_def: ImportDef) -> Result<pl::ImportDef, Error> {
        let (fq_ident, indirections) = self.resolve_ident(import_def.name)?;
        if !indirections.is_empty() {
            return Err(Error::new_simple(
                "Import can only reference modules and declarations",
            ));
        }
        Ok(ImportDef {
            name: fq_ident,
            alias: import_def.alias,
        })
    }
}

impl pl::PlFold for DeclRefResolver<'_> {
    fn fold_expr(&mut self, expr: pl::Expr) -> Result<pl::Expr> {
        Ok(match expr.kind {
            pl::ExprKind::Ident(ident) => {
                let (ident, indirections) = self.resolve_ident(ident).with_span(expr.span)?;

                // TODO: hack for until indirections are implemented: convert back to ident
                let ident = ast::Ident::from_path(ident.into_iter().chain(indirections).collect());

                // for indirection in indirections {
                //     r = pl::Expr::new(pl::ExprKind::Indirection {
                //         base: Box::new(r),
                //         field: indirection,
                //     })
                // }
                pl::Expr {
                    kind: pl::ExprKind::Ident(ident),
                    ..expr
                }
            }
            _ => pl::Expr {
                kind: pl::fold_expr_kind(self, expr.kind)?,
                ..expr
            },
        })
    }

    fn fold_type(&mut self, ty: ast::Ty) -> Result<ast::Ty> {
        Ok(match ty.kind {
            ast::TyKind::Ident(ident) => {
                let (ident, indirections) = self.resolve_ident(ident).with_span(ty.span)?;

                if !indirections.is_empty() {
                    return Err(
                        Error::new_simple("types are not allowed indirections").with_span(ty.span)
                    );
                }

                ast::Ty {
                    kind: ast::TyKind::Ident(ident),
                    ..ty
                }
            }
            _ => pl::fold_type(self, ty)?,
        })
    }
}

impl DeclRefResolver<'_> {
    /// Returns resolved fully-qualified ident and a list of indirections
    fn resolve_ident(&mut self, ident: ast::Ident) -> Result<(ast::Ident, Vec<String>)> {
        // this is the name we are looking for
        let name = ident.iter().next().unwrap();

        let decl = find_lookup_base(&self.root.module, self.decl_module_path, name);
        let (ident, indirections) = if let Some((module, mod_path)) = decl {
            // module found

            // now find the decl within that module
            let (_, path, indirections) = lookup_within_module(module, ident)?;

            // prepend the ident with the module path
            // this will make this ident a fully-qualified ident
            let mut fq_ident = mod_path;
            fq_ident.extend(path);
            let fq_ident = ast::Ident::from_path(fq_ident);

            self.refs.push(fq_ident.clone());

            (fq_ident, indirections)
        } else {
            // cannot find module, so this must be a ref to a local var + indirections
            let mut steps = ident.into_iter();
            let first = steps.next().unwrap();
            let indirections = steps.collect_vec();
            (ast::Ident::from_name(first), indirections)
        };
        Ok((ident, indirections))
    }
}

// Find declaration by name, starting in the current module,
// then to parent, then grandparent until root.
fn find_lookup_base<'m>(
    root_mod: &'m decl::Module,
    current_mod_path: &[String],
    name: &String,
) -> Option<(&'m decl::Module, Vec<String>)> {
    let mut module_path = root_mod
        .get_module_path(current_mod_path)
        .unwrap_or_else(|| panic!("path does not exist: {:?}", current_mod_path));

    let mut path = current_mod_path;

    while let Some(module) = module_path.pop() {
        if let Some((module, redirects)) = module_contains(module, name) {
            let mut path = path.to_vec();
            path.extend(redirects);
            return Some((module, path));
        }

        if !path.is_empty() {
            path = &path[0..(path.len() - 1)];
        }
    }

    None
}

/// Does module contains a name? If yes, return the module and redirection path to it.
fn module_contains<'m>(
    module: &'m decl::Module,
    name: &String,
) -> Option<(&'m decl::Module, Vec<String>)> {
    // look into the module (obviously)
    if module.names.contains_key(name) {
        return Some((module, vec![]));
    }

    // also look into all redirected modules, recursively
    for redirect in &module.redirects {
        let Some(redirected) = module.get(redirect).and_then(|x| x.kind.as_module()) else {
            continue;
        };

        let Some((module, inner_redirects)) = module_contains(redirected, name) else {
            continue;
        };

        // redirect matched: combine the redirected paths
        let redirect = (redirect.clone().into_iter())
            .chain(inner_redirects)
            .collect_vec();
        return Some((module, redirect));
    }

    None
}

fn lookup_within_module(
    module: &decl::Module,
    ident_within: ast::Ident,
) -> Result<(&decl::Decl, Vec<String>, Vec<String>)> {
    let mut steps = ident_within.into_iter().collect_vec();

    let mut module = module;
    for i in 0..steps.len() {
        let decl = match module.names.get(&steps[i]) {
            Some(decl) => decl,
            _ => {
                if let Some(decl) = &module.infer_decl {
                    // declaration was not found, but this module will infer the decl
                    decl.as_ref()
                } else {
                    // declaration not found
                    return Err(Error::new_simple(format!("`{}` does not exist", steps[i])));
                }
            }
        };

        match &decl.kind {
            decl::DeclKind::Module(inner) => {
                module = inner;
            }
            _ => {
                let indirections = steps.drain((i + 1)..).collect_vec();
                return Ok((decl, steps, indirections));
            }
        }
    }

    Err(Error::new_simple("direct references modules not allowed"))
}
