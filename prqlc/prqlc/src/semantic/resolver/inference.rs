use crate::ast::{Ident, Ty, TyKind, TyTupleField};
use crate::codegen::write_ty;
use crate::ir::decl::{Decl, DeclKind, InferTarget, Module, TableDecl, TableExpr};
use crate::semantic::NS_GENERIC;
use crate::{Error, Result, WithErrorInfo};

use super::Resolver;

impl Resolver<'_> {
    /// Create a declaration of [original] from template provided by declaration of [infer_ident].
    pub fn infer_decl(&mut self, infer_ident: Ident, original: &Ident) -> Result<Ident, String> {
        let infer = self.root_mod.module.get(&infer_ident).unwrap();
        let infer_target = infer.kind.as_infer().unwrap();

        // prepare the new declaration
        let new_decl = match infer_target {
            InferTarget::DatabaseModule => Decl::from(DeclKind::Module(Module::new_database())),
            InferTarget::Table => {
                // TODO: move this inference into the first pass (resolve_decl)

                // generate a new global generic type argument
                let ident = self.init_new_global_generic();

                // prepare the table type
                let generic_param = Ty::new(TyKind::Ident(ident));
                let relation = Ty::relation(vec![TyTupleField::Unpack(Some(generic_param))]);

                // create the table decl
                Decl::from(DeclKind::TableDecl(TableDecl {
                    ty: Some(relation),
                    expr: TableExpr::LocalTable,
                }))
            }
            InferTarget::TupleField { .. } => Decl::from(DeclKind::TupleField),
        };

        // find the module to insert into
        let module_ident = infer_ident.pop().unwrap();
        let module = self.root_mod.module.get_mut(&module_ident).unwrap();
        let module = module.kind.as_module_mut().unwrap();

        // insert
        module.names.insert(original.name.clone(), new_decl);

        // TODO: if this was inferred to be a field of a tuple, go and infer table columns

        Ok(module_ident + Ident::from_name(original.name.clone()))
    }

    fn init_new_global_generic(&mut self) -> Ident {
        let a_unique_number = self.id.gen();
        let param_name = format!("G{a_unique_number}");
        let ident = Ident::from_path(vec![NS_GENERIC.to_string(), param_name]);
        let decl = Decl::from(DeclKind::GenericParam(None));

        self.root_mod.module.insert(ident.clone(), decl).unwrap();
        ident
    }

    /// When we refer to `Generic.my_field`, this function pushes information that `Generic`
    /// is a tuple with a field `my_field` into the generic type argument.
    pub fn infer_tuple_field_of_generic(
        &mut self,
        ident_of_generic: &Ident,
        field_name: &str,
    ) -> Result<(usize, &TyTupleField)> {
        // generate the type of inferred field (to be an unknown type - a new generic)
        // (this has to be done early in this function since we borrow self later)
        let ty_of_field = self.init_new_global_generic();

        let ident = ident_of_generic;
        let generic_decl = self.root_mod.module.get_mut(ident).unwrap();
        let inferred_type = generic_decl.kind.as_generic_param_mut().unwrap();

        // if there is no candidates yet, propose a new type
        if inferred_type.is_none() {
            *inferred_type = Some((Ty::new(TyKind::Tuple(vec![])), None));
        }
        let inferred_type = inferred_type.as_mut().unwrap();

        // unpack the generic as a tuple
        if !inferred_type.0.kind.is_tuple() {
            return Err(Error::new_simple(format!(
                "cannot lookup fields in type {}",
                write_ty(&inferred_type.0)
            ))
            .push_hint("inferring type of a generic argument"));
        };
        let fields_of_generic = inferred_type.0.kind.as_tuple_mut().unwrap();

        // push the type info into the candidate for the generic type
        fields_of_generic.push(TyTupleField::Single(
            Some(field_name.to_string()),
            Some(Ty::new(TyKind::Ident(ty_of_field))),
        ));
        Ok((
            fields_of_generic.len() - 1, // position within the generic
            fields_of_generic.last().unwrap(),
        ))
    }

    /// Converts a identifier that points to a table declaration to lineage of that table.
    pub fn ty_of_table_decl(&mut self, table_fq: &Ident) -> Ty {
        let table_decl = self.root_mod.module.get(table_fq).unwrap();
        let TableDecl { ty, .. } = table_decl.kind.as_table_decl().unwrap();

        ty.clone()
            .expect("a referenced relation to have its type resolved")
    }
}
