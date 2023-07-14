use std::collections::HashSet;

use anyhow::Result;
use itertools::Itertools;

use prql_ast::expr::Ident;

use crate::{
    ast::pl::{
        expr::{Expr, ExprKind},
        stmt::Annotation,
        types::{TupleField, Ty, TyKind},
    },
    error::WithErrorInfo,
    semantic::{
        context::{Decl, DeclKind, TableDecl, TableExpr},
        Context, Module, NS_DEFAULT_DB, NS_INFER, NS_INFER_MODULE, NS_SELF, NS_STD, NS_THIS,
    },
    Error,
};

impl Context {
    pub(super) fn declare(
        &mut self,
        ident: Ident,
        decl: DeclKind,
        id: Option<usize>,
        annotations: Vec<Annotation>,
    ) -> Result<()> {
        let existing = self.root_mod.get(&ident);
        if existing.is_some() {
            return Err(Error::new_simple(format!("duplicate declarations of {ident}")).into());
        }

        {
            // HACK: because we are creating default_db module prior to std,
            // we cannot provide type annotations correctly.
            // Here we are adding-in these annotations when they are defined in std module.
            if ident.path == [NS_STD] && ident.name == "scalar" {
                let val = decl.as_expr().unwrap().kind.as_type().unwrap().clone();

                let default_db_infer = Ident::from_path(vec![NS_DEFAULT_DB, NS_INFER]);
                let infer = self.root_mod.get_mut(&default_db_infer).unwrap();
                let infer_table = infer.kind.as_infer_mut().unwrap();
                let infer_table = infer_table.as_table_decl_mut().unwrap();
                let infer_ty = infer_table.ty.as_mut().unwrap();
                let infer_field = infer_ty.as_relation_mut().unwrap().get_mut(0).unwrap();
                let (ty, _) = infer_field.as_all_mut().unwrap();
                *ty = Some(val);
            }
        }

        let decl = Decl {
            kind: decl,
            declared_at: id,
            order: 0,
            annotations,
        };
        self.root_mod.insert(ident, decl).unwrap();
        Ok(())
    }

    pub(super) fn prepare_expr_decl(&mut self, value: Box<Expr>) -> DeclKind {
        match &value.ty {
            Some(ty) if ty.is_relation() => {
                let mut ty = ty.clone();
                ty.flatten_tuples();
                let ty = Some(ty);

                let expr = TableExpr::RelationVar(value);
                DeclKind::TableDecl(TableDecl { expr, ty })
            }
            _ => DeclKind::Expr(value),
        }
    }

    pub(super) fn resolve_ident(
        &mut self,
        ident: &Ident,
        default_namespace: Option<&String>,
    ) -> Result<Ident, Error> {
        // special case: wildcard
        if ident.name.contains('*') {
            if ident.name != "*" {
                return Err(Error::new_simple(
                    "Unsupported feature: advanced wildcard column matching",
                ));
            }
            return self
                .resolve_ident_wildcard(ident)
                .map_err(Error::new_simple);
        }

        // base case: direct lookup
        let decls = self.root_mod.lookup(ident);
        match decls.len() {
            // no match: try match *
            0 => {}

            // single match, great!
            1 => return Ok(decls.into_iter().next().unwrap()),

            // ambiguous
            _ => return Err(ambiguous_error(decls, None)),
        }

        let ident = if let Some(default_namespace) = default_namespace {
            let ident = ident.clone().prepend(vec![default_namespace.clone()]);

            let decls = self.root_mod.lookup(&ident);
            match decls.len() {
                // no match: try match *
                0 => ident,

                // single match, great!
                1 => return Ok(decls.into_iter().next().unwrap()),

                // ambiguous
                _ => return Err(ambiguous_error(decls, None)),
            }
        } else {
            ident.clone()
        };

        // fallback case: try to match with NS_INFER and infer the declaration from the original ident.
        match self.resolve_ident_fallback(ident, NS_INFER) {
            // The declaration and all needed parent modules were created
            // -> just return the fq ident
            Ok(inferred_ident) => Ok(inferred_ident),

            // Was not able to infer.
            Err(None) => Err(Error::new_simple("Unknown name".to_string())),
            Err(Some(msg)) => Err(msg),
        }
    }

    /// Try lookup of the ident with name replaced. If unsuccessful, recursively retry parent ident.
    fn resolve_ident_fallback(
        &mut self,
        ident: Ident,
        name_replacement: &'static str,
    ) -> Result<Ident, Option<Error>> {
        let infer_ident = ident.clone().with_name(name_replacement);

        // lookup of infer_ident
        let mut decls = self.root_mod.lookup(&infer_ident);

        if decls.is_empty() {
            if let Some(parent) = infer_ident.clone().pop() {
                // try to infer parent
                let _ = self.resolve_ident_fallback(parent, NS_INFER_MODULE)?;

                // module was successfully inferred, retry the lookup
                decls = self.root_mod.lookup(&infer_ident)
            }
        }

        match decls.len() {
            1 => {
                // single match, great!
                let infer_ident = decls.into_iter().next().unwrap();
                self.infer_decl(infer_ident, &ident)
                    .map_err(|x| Some(Error::new_simple(x)))
            }
            0 => Err(None),
            _ => Err(Some(ambiguous_error(decls, Some(&ident.name)))),
        }
    }

    /// Create a declaration of [original] from template provided by declaration of [infer_ident].
    fn infer_decl(&mut self, infer_ident: Ident, original: &Ident) -> Result<Ident, String> {
        let infer = self.root_mod.get(&infer_ident).unwrap();
        let mut infer_default = *infer.kind.as_infer().cloned().unwrap();

        if let DeclKind::Module(new_module) = &mut infer_default {
            // Modules are inferred only for database inference.
            // Because we want to infer database modules that nested arbitrarily deep,
            // we cannot store the template in DeclKind::Infer, but we override it here.
            *new_module = Module::new_database();
        }

        let module_ident = infer_ident.pop().unwrap();
        let module = self.root_mod.get_mut(&module_ident).unwrap();
        let module = module.kind.as_module_mut().unwrap();

        // insert default
        module
            .names
            .insert(original.name.clone(), Decl::from(infer_default));

        // infer table columns
        if let Some(decl) = module.names.get(NS_SELF).cloned() {
            if let DeclKind::InstanceOf(table_ident) = decl.kind {
                log::debug!("inferring {original} to be from table {table_ident}");
                self.infer_table_column(&table_ident, &original.name)?;
            }
        }

        Ok(module_ident + Ident::from_name(original.name.clone()))
    }

    fn resolve_ident_wildcard(&mut self, ident: &Ident) -> Result<Ident, String> {
        let mod_ident = self.find_module_of_wildcard(ident)?;
        let mod_decl = self.root_mod.get(&mod_ident).unwrap();

        let instance_of = mod_decl.kind.as_instance_of().unwrap();
        let decl = self.root_mod.get(instance_of).unwrap();
        let decl = decl.kind.as_table_decl().unwrap();

        let fields = decl.ty.clone().unwrap().into_relation().unwrap();

        // This is just a workaround to return an Ident from this function.
        // We wrap the expr into DeclKind::Expr and save it into context.
        let cols_expr = Expr {
            ty: Some(Ty {
                instance_of: Some(instance_of.clone()),
                ..Ty::from(TyKind::Tuple(fields))
            }),
            ..Expr::new(ExprKind::TupleFields(vec![]))
        };
        let cols_expr = DeclKind::Expr(Box::new(cols_expr));
        let save_as = "_wildcard_match";
        self.root_mod
            .names
            .insert(save_as.to_string(), cols_expr.into());

        // Then we can return ident to that decl.
        Ok(Ident::from_name(save_as))
    }

    fn find_module_of_wildcard(&self, wildcard_ident: &Ident) -> Result<Ident, String> {
        let mod_ident = wildcard_ident.clone().pop().unwrap() + Ident::from_name(NS_SELF);

        let fq_mod_idents = self.root_mod.lookup(&mod_ident);

        // TODO: gracefully handle this
        Ok(fq_mod_idents.into_iter().exactly_one().unwrap())
    }

    fn infer_table_column(&mut self, table_ident: &Ident, col_name: &str) -> Result<(), String> {
        let table = self.root_mod.get_mut(table_ident).unwrap();
        let table_decl = table.kind.as_table_decl_mut().unwrap();

        let Some(columns) = table_decl.ty.as_mut().and_then(|t| t.as_relation_mut()) else {
            return Err(format!("Variable {table_ident:?} is not a relation."));
        };

        let ty = if let Some(all) = columns.iter_mut().find_map(|c| c.as_all_mut()) {
            all.1.insert(Ident::from_name(col_name));

            // Use the type from TupleField::All for the inferred field.
            all.0.clone()
        } else {
            return Err(format!("Table {table_ident:?} does not have wildcard."));
        };

        let exists = columns.iter().any(|c| match c {
            TupleField::Single(Some(n), _) => n == col_name,
            _ => false,
        });
        if exists {
            return Ok(());
        }

        columns.push(TupleField::Single(Some(col_name.to_string()), ty));

        // also add into input tables of this table expression
        if let TableExpr::RelationVar(expr) = &table_decl.expr {
            if let Some(ty) = &expr.ty {
                if let Some(fields) = ty.as_relation() {
                    let wildcard_inputs = (fields.iter()).filter_map(|c| c.as_all()).collect_vec();

                    match wildcard_inputs.len() {
                        0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
                        1 => {
                            let (wildcard_ty, _) = wildcard_inputs.into_iter().next().unwrap();
                            let wildcard_ty = wildcard_ty.as_ref().unwrap();
                            let table_fq = wildcard_ty.instance_of.clone().unwrap();

                            self.infer_table_column(&table_fq, col_name)?;
                        }
                        _ => {
                            return Err(format!("Cannot infer where {table_ident}.{col_name} is from. It could be any of {wildcard_inputs:?}"))
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn ambiguous_error(idents: HashSet<Ident>, replace_name: Option<&String>) -> Error {
    let all_this = idents.iter().all(|d| d.starts_with_part(NS_THIS));

    let mut chunks = Vec::new();
    for mut ident in idents {
        if all_this {
            let (_, rem) = ident.pop_front();
            if let Some(rem) = rem {
                ident = rem;
            } else {
                continue;
            }
        }

        if let Some(name) = replace_name {
            ident.name = name.clone();
        }
        chunks.push(ident.to_string());
    }
    chunks.sort();
    let hint = format!("could be any of: {}", chunks.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}
