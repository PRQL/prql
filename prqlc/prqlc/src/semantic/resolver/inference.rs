use prqlc_ast::error::WithErrorInfo;

use crate::ast::{Ident, Ty, TyKind, TyTupleField};
use crate::codegen::write_ty;
use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::IndirectionKind;
use crate::semantic::NS_GENERIC;
use crate::{Error, Result, Span};

use super::Resolver;

impl Resolver<'_> {
    pub fn init_new_global_generic(&mut self, prefix: &str) -> Ident {
        let a_unique_number = self.id.gen();
        let param_name = format!("{prefix}{a_unique_number}");
        let ident = Ident::from_path(vec![NS_GENERIC.to_string(), param_name]);
        let decl = Decl::from(DeclKind::GenericParam(None));

        self.root_mod.module.insert(ident.clone(), decl).unwrap();
        ident
    }

    /// When we refer to `Generic.my_field`, this function pushes information that `Generic`
    /// is a tuple with a field `my_field` into the generic type argument.
    ///
    /// Contract:
    /// - ident must be fq ident of a generic type param,
    /// - generic candidate either must not exist yet or be a tuple,
    /// - if it is a tuple, it must already contain the indirection target.
    pub fn infer_tuple_field_of_generic(
        &mut self,
        ident_of_generic: &Ident,
        indirection: &IndirectionKind,
        pos_offset: usize,
    ) -> (usize, Option<Ty>) {
        // generate the type of inferred field (to be an unknown type - a new generic)
        // (this has to be done early in this function since we borrow self later)
        let ty_of_field = self.init_new_global_generic("F");
        let ty = Ty::new(TyKind::Ident(ty_of_field));

        let ident = ident_of_generic;
        let generic_decl = self.root_mod.module.get_mut(ident).unwrap();
        let candidate = generic_decl.kind.as_generic_param_mut().unwrap();

        // if there is no candidate yet, propose a new tuple type
        if candidate.is_none() {
            *candidate = Some((Ty::new(TyKind::Tuple(vec![])), None));
        }
        let (candidate_ty, _) = candidate.as_mut().unwrap();
        let candidate_fields = candidate_ty.kind.as_tuple_mut().unwrap();

        // create new field(s)
        match indirection {
            IndirectionKind::Name(field_name) => {
                candidate_fields.push(TyTupleField::Single(
                    Some(field_name.clone()),
                    Some(ty.clone()),
                ));

                let pos_within_candidate = candidate_fields.len() - 1;
                (pos_offset + pos_within_candidate, Some(ty))
            }
            IndirectionKind::Position(pos) => {
                let pos = *pos as usize;
                let pos_within_candidate = pos - pos_offset;

                // fill-in padding fields
                for _ in 0..(pos_within_candidate - candidate_fields.len()) {
                    // TODO: these should all be generics
                    candidate_fields.push(TyTupleField::Single(None, None));
                }

                // push the actual field
                candidate_fields.push(TyTupleField::Single(None, Some(ty.clone())));
                (pos, Some(ty))
            }
        }
    }

    pub fn infer_type_of_generic(
        &mut self,
        ident_of_generic: &Ident,
        ty: Ty,
        span: Option<Span>,
    ) -> Result<()> {
        if let TyKind::Ident(ty_ident) = &ty.kind {
            if ty_ident == ident_of_generic {
                // don't infer that T is T
                return Ok(());
            }
        }

        log::debug!("inferring that {ident_of_generic:?} is {}", write_ty(&ty));

        let Some(decl) = self.get_ident(ident_of_generic, true) else {
            return Err(Error::new_assert("type not found"));
        };
        let DeclKind::GenericParam(inferred_type) = &mut decl.kind else {
            return Err(Error::new_assert("expected a generic type param")
                .push_hint(format!("found {:?}", decl.kind)));
        };

        if let Some(existing) = inferred_type {
            let existing = existing.clone();
            return self.validate_type(&ty, &existing.0, existing.1, &|| None);
        }

        *inferred_type = Some((ty, span));
        Ok(())
    }
}
