use itertools::Itertools;

use crate::ir::decl::{self, Decl, DeclKind, InferTarget};
use crate::ir::pl::{self, PlFold};
use crate::semantic::{NS_DEFAULT_DB, NS_GENERIC, NS_INFER, NS_LOCAL, NS_STD, NS_THIS};
use crate::utils::IdGenerator;
use crate::{pr, utils};
use crate::{Error, Result, WithErrorInfo};

/// Runs name resolution for global names - names that refer to declarations.
///
/// Keeps track of all inter-declaration references.
/// Returns a resolution order.
pub fn resolve_decl_refs(root: &mut decl::RootModule) -> Result<Vec<pl::Ident>> {
    // resolve inter-declaration references
    let refs = {
        let mut r = ModuleRefResolver {
            root,
            generic_name: IdGenerator::new(),
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
    let order = utils::toposort::<pr::Ident>(&refs, None);

    if let Some(order) = order {
        Ok(order.into_iter().cloned().collect_vec())
    } else {
        todo!("error for a cyclic references between expressions")
    }
}

/// Traverses module tree and runs name resolution on each of the declarations.
/// Collects references of each declaration.
struct ModuleRefResolver<'a> {
    root: &'a mut decl::RootModule,
    generic_name: IdGenerator<usize>,
    current_path: Vec<String>,

    // TODO: maybe make these ids, instead of Ident?
    refs: Vec<(pr::Ident, Vec<pr::Ident>)>,
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
            let mut r = NameResolver {
                root: self.root,
                generic_name: &mut self.generic_name,
                decl_module_path: &path[0..(path.len() - 1)],
                refs: Vec::new(),
            };

            let stmt = decl.kind.into_unresolved().unwrap();
            let stmt = r.fold_stmt_kind(stmt).with_span_fallback(span)?;
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

/// Traverses AST and resolves all global (non-local) identifiers.
struct NameResolver<'a> {
    root: &'a mut decl::RootModule,
    generic_name: &'a mut IdGenerator<usize>,
    decl_module_path: &'a [String],
    refs: Vec<pl::Ident>,
}

impl NameResolver<'_> {
    fn fold_stmt_kind(&mut self, stmt: pl::StmtKind) -> Result<pl::StmtKind> {
        Ok(match stmt {
            pl::StmtKind::QueryDef(_) => stmt,
            pl::StmtKind::VarDef(var_def) => pl::StmtKind::VarDef(self.fold_var_def(var_def)?),
            pl::StmtKind::TypeDef(ty_def) => pl::StmtKind::TypeDef(self.fold_type_def(ty_def)?),
            pl::StmtKind::ImportDef(import_def) => {
                pl::StmtKind::ImportDef(self.fold_import_def(import_def)?)
            }
            pl::StmtKind::ModuleDef(_) => unreachable!(),
        })
    }

    fn fold_import_def(&mut self, import_def: pl::ImportDef) -> Result<pl::ImportDef, Error> {
        let (fq_ident, indirections) = self.resolve_ident(import_def.name)?;
        if !indirections.is_empty() {
            return Err(Error::new_simple(
                "Import can only reference modules and declarations",
            ));
        }
        if fq_ident.is_empty() {
            log::debug!("resolved type ident to : {fq_ident:?} + {indirections:?}");
            return Err(Error::new_simple("invalid type name"));
        }
        Ok(pl::ImportDef {
            name: pr::Ident::from_path(fq_ident),
            alias: import_def.alias,
        })
    }
}

impl pl::PlFold for NameResolver<'_> {
    fn fold_expr(&mut self, expr: pl::Expr) -> Result<pl::Expr> {
        // Convert indirections into ident, since the algo below works with
        // full idents and not indirections.
        // We could change that to work with indirections, but then we'd
        // need to change how idents in types and imports are resolved.
        let expr = push_indirections_into_ident(expr);

        Ok(match expr.kind {
            pl::ExprKind::Ident(ident) => {
                let (ident, indirections) = self.resolve_ident(ident).with_span(expr.span)?;
                // TODO: can this ident have length 0?

                let mut kind = pl::ExprKind::Ident(pr::Ident::from_path(ident));
                for indirection in indirections {
                    let mut e = pl::Expr::new(kind);
                    e.span = expr.span;
                    kind = pl::ExprKind::Indirection {
                        base: Box::new(e),
                        field: pl::IndirectionKind::Name(indirection),
                    };
                }

                pl::Expr { kind, ..expr }
            }
            _ => pl::Expr {
                kind: pl::fold_expr_kind(self, expr.kind)?,
                ..expr
            },
        })
    }

    fn fold_type(&mut self, ty: pr::Ty) -> Result<pr::Ty> {
        Ok(match ty.kind {
            pr::TyKind::Ident(ident) => {
                let (ident, indirections) = self.resolve_ident(ident).with_span(ty.span)?;

                if !indirections.is_empty() {
                    log::debug!("resolved type ident to : {ident:?} + {indirections:?}");
                    return Err(
                        Error::new_simple("types are not allowed indirections").with_span(ty.span)
                    );
                }

                if ident.is_empty() {
                    log::debug!("resolved type ident to : {ident:?} + {indirections:?}");
                    return Err(Error::new_simple("invalid type name").with_span(ty.span));
                }

                pr::Ty {
                    kind: pr::TyKind::Ident(pr::Ident::from_path(ident)),
                    ..ty
                }
            }
            _ => pl::fold_type(self, ty)?,
        })
    }
}

/// Converts `Indirection { base: Ident(x), field: y }` into `Ident(x.y)`.
fn push_indirections_into_ident(mut expr: pl::Expr) -> pl::Expr {
    let mut indirections = Vec::new();
    while let pl::ExprKind::Indirection {
        base,
        field: pl::IndirectionKind::Name(name),
    } = expr.kind
    {
        indirections.push((name, expr.span, expr.alias, expr.flatten));
        expr = *base;
    }

    if let pl::ExprKind::Ident(ident) = &mut expr.kind {
        for (part, span, alias, flatten) in indirections.into_iter().rev() {
            ident.push(part);
            expr.span = pr::Span::merge_opt(expr.span, span);
            expr.alias = alias.or(expr.alias);
            expr.flatten = flatten;
        }
    } else {
        // this is not on an ident - we have to revert it
        for (name, span, alias, flatten) in indirections {
            expr = pl::Expr::new(pl::ExprKind::Indirection {
                base: Box::new(expr),
                field: pl::IndirectionKind::Name(name),
            });
            expr.span = span;
            expr.alias = alias;
            expr.flatten = flatten;
        }
    }
    expr
}

impl NameResolver<'_> {
    /// Returns resolved fully-qualified ident and a list of indirections
    fn resolve_ident(&mut self, mut ident: pr::Ident) -> Result<(Vec<String>, Vec<String>)> {
        // this is the name we are looking for
        let first = ident.iter().next().unwrap();
        let mod_path = match first.as_str() {
            "project" => Some(vec![]),
            "module" => Some(self.decl_module_path.to_vec()),
            "super" => {
                let mut path = self.decl_module_path.to_vec();
                path.pop();
                Some(path)
            }

            NS_STD => Some(vec![NS_STD.to_string()]),
            NS_DEFAULT_DB => Some(vec![NS_DEFAULT_DB.to_string()]),
            NS_THIS => Some(vec![NS_LOCAL.to_string(), NS_THIS.to_string()]),
            "prql" => Some(vec![NS_STD.to_string(), "prql".to_string()]),

            // transforms
            "from" |
            "select" |
            "filter" |
            "derive" |
            "aggregate" |
            "sort" |
            "take" |
            "join" |
            "group" |
            "window" |
            "append" |
            "intersect" |
            "remove" |
            "loop" |
            // agg
            "min" |
            "max" |
            "sum" |
            "average" |
            "stddev" |
            "all" |
            "any" |
            "concat_array" |
            "count" |
            "count_distinct" |
            "lag" |
            "lead" |
            "first" |
            "last" |
            "rank" |
            "rank_dense" |
            "row_number" |
            // utils
            "in" |
            "as" => {
                ident = ident.prepend(vec![NS_STD.to_string()]);
                Some(vec![NS_STD.to_string()])
            }

            _ => None,
        };
        let mod_decl = mod_path
            .as_ref()
            .and_then(|p| self.root.module.get_submodule_mut(p));

        // let decl = find_lookup_base(&self.root.module, self.decl_module_path, name);
        Ok(if let Some(module) = mod_decl {
            let mod_path = mod_path.unwrap();
            // module found

            // now find the decl within that module
            if let Some(ident_within) = ident.pop_front().1 {
                let mut module_lookup = ModuleLookup::new(self.generic_name);

                let (path, indirections) = module_lookup.run(module, ident_within)?;

                // prepend the ident with the module path
                // this will make this ident a fully-qualified ident
                let mut fq_ident = mod_path;
                fq_ident.extend(path);

                self.refs.push(pr::Ident::from_path(fq_ident.clone()));

                module_lookup.finish(self.root);
                (fq_ident, indirections)
            } else {
                // there is no inner ident - we return the fq path to the module
                (mod_path, vec![])
            }
        } else {
            // cannot find module, so this must be a ref to a local var + indirections
            let mut steps = ident.into_iter();
            let first = steps.next().unwrap();
            let indirections = steps.collect_vec();
            (vec![NS_LOCAL.to_string(), first], indirections)
        })
    }
}

struct ModuleLookup<'a> {
    generic_name: &'a mut IdGenerator<usize>,

    generated_generics: Vec<(String, Decl)>,
}

impl<'a> ModuleLookup<'a> {
    fn new(generic_name: &'a mut IdGenerator<usize>) -> Self {
        ModuleLookup {
            generic_name,
            generated_generics: Vec::new(),
        }
    }

    fn run(
        &mut self,
        module: &mut decl::Module,
        ident_within: pr::Ident,
    ) -> Result<(Vec<String>, Vec<String>)> {
        let mut steps = ident_within.into_iter().collect_vec();

        let mut module = module;
        for i in 0..steps.len() {
            let is_last = i == steps.len() - 1;

            let decl = self.run_step(module, &steps[i], is_last)?;
            if let decl::DeclKind::Module(inner) = &mut decl.kind {
                module = inner;
                continue;
            } else {
                // we've found a declaration that is not a module:
                // this and preceding steps are identifier, steps following are indirections
                let indirections = steps.drain((i + 1)..).collect_vec();
                return Ok((steps, indirections));
            }
        }

        Err(Error::new_simple("direct references modules not allowed"))
    }

    fn run_step<'m>(
        &mut self,
        module: &'m mut decl::Module,
        step: &str,
        is_last: bool,
    ) -> Result<&'m mut decl::Decl> {
        if module.names.contains_key(step) {
            return Ok(module.names.get_mut(step).unwrap());
        }

        let infer_decl = module.names.get(NS_INFER);
        let can_infer_tables = infer_decl
            .and_then(|i| i.kind.as_infer())
            .map_or(false, |i| matches!(i, InferTarget::Table));
        if !can_infer_tables {
            return Err(Error::new_simple(format!("`{}` does not exist", step)));
        }

        let decl = if is_last {
            // infer a table

            // generate a new global generic type argument
            let ident = self.init_new_global_generic();

            // prepare the table type
            let generic_param = pr::Ty::new(pr::TyKind::Ident(ident));
            let relation = pr::Ty::relation(vec![pr::TyTupleField::Unpack(Some(generic_param))]);

            // create the table decl
            decl::Decl::from(decl::DeclKind::Expr(Box::new(pl::Expr {
                ty: Some(relation),
                ..pl::Expr::new(pl::ExprKind::Param("".to_string()))
            })))
        } else {
            // infer a database module
            Decl::from(DeclKind::Module(decl::Module::new_database()))
        };

        module.names.insert(step.to_string(), decl);
        Ok(module.names.get_mut(step).unwrap())
    }

    fn init_new_global_generic(&mut self) -> pr::Ident {
        let a_unique_number = self.generic_name.gen();
        let param_name = format!("T{a_unique_number}");
        let ident = pr::Ident::from_path(vec![NS_GENERIC, &param_name]);
        let decl = Decl::from(DeclKind::GenericParam(None));

        self.generated_generics.push((param_name, decl));
        ident
    }

    fn finish(self, root: &mut decl::RootModule) {
        let generic_mod = root
            .module
            .names
            .entry(NS_GENERIC.to_string())
            .or_insert_with(|| decl::Decl::from(decl::DeclKind::Module(decl::Module::default())));
        let generic_mod = generic_mod.kind.as_module_mut().unwrap();

        for (name, decl) in self.generated_generics {
            generic_mod.names.insert(name, decl);
        }
    }
}
