use crate::pr::{Ident, Ty, TyKind, TyTupleField};
use crate::codegen::write_ty;
use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl::IndirectionKind;
use crate::semantic::NS_GENERIC;
use crate::{Error, Result, Span, WithErrorInfo};

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

    /// For a given generic, infer that it must be of type `ty`.
    pub fn infer_generic_as_ty(
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

        let Some(decl) = self.get_ident_mut(ident_of_generic) else {
            return Err(Error::new_assert("type not found"));
        };
        let DeclKind::GenericParam(candidate) = &mut decl.kind else {
            return Err(Error::new_assert("expected a generic type param")
                .push_hint(format!("found {:?}", decl.kind)));
        };

        if let Some((candidate, _)) = candidate {
            // validate that ty has all fields of the candidate
            let candidate = candidate.clone();
            self.validate_type(&ty, &candidate, span, &|| None)?;

            // ty has all fields of the candidate, but it might have additional ones
            // so we need to add all of them to the candidate
            // (we need to get the candidate ref again, since we need &mut self for validate_type)
            let Some(decl) = self.get_ident_mut(ident_of_generic) else {
                unreachable!()
            };
            let DeclKind::GenericParam(Some(candidate)) = &mut decl.kind else {
                unreachable!()
            };

            candidate.0.kind = ty.kind; // maybe merge the fields here?
            return Ok(());
        }

        *candidate = Some((ty, span));
        Ok(())
    }

    /// When we refer to `Generic.my_field`, this function pushes information that `Generic`
    /// is a tuple with a field `my_field` into the generic type argument.
    ///
    /// Contract:
    /// - ident must be fq ident of a generic type param,
    /// - generic candidate either must not exist yet or be a tuple,
    /// - if it is a tuple, it must not yet contain the indirection target.
    pub fn infer_generic_as_tuple(
        &mut self,
        ident_of_generic: &Ident,
        indirection: IndirectionKind,
    ) -> (usize, Ty) {
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
                candidate_fields.push(TyTupleField::Single(Some(field_name), Some(ty.clone())));

                let pos_within_candidate = candidate_fields.len() - 1;
                (pos_within_candidate, ty)
            }
            IndirectionKind::Position(pos) => {
                let pos = pos as usize;

                // fill-in padding fields
                for _ in 0..(pos - candidate_fields.len()) {
                    // TODO: these should all be generics
                    candidate_fields.push(TyTupleField::Single(None, None));
                }

                // push the actual field
                candidate_fields.push(TyTupleField::Single(None, Some(ty.clone())));
                (pos, ty)
            }
        }
    }

    pub fn infer_generic_as_array(
        &mut self,
        ident_of_generic: &Ident,
        span: Option<Span>,
    ) -> Result<Ty> {
        // generate the type of array items (to be an unknown type - a new generic)
        // (this has to be done early in this function since we borrow self later)
        let items_ty = self.init_new_global_generic("A");
        let items_ty = Ty::new(TyKind::Ident(items_ty));

        let ident = ident_of_generic;
        let generic_decl = self.root_mod.module.get_mut(ident).unwrap();
        let candidate = generic_decl.kind.as_generic_param_mut().unwrap();

        // if there is no candidate yet, propose a new tuple type
        if let Some((candidate, _)) = candidate.as_mut() {
            if let TyKind::Array(items_ty) = &candidate.kind {
                // ok, we already know it is an array
                Ok(*items_ty.clone())
            } else {
                // nope
                Err(Error::new_simple(format!(
                    "generic type argument {} needs to be an array",
                    ident_of_generic
                ))
                .push_hint(format!("existing candidate: {}", write_ty(candidate))))
            }
        } else {
            *candidate = Some((Ty::new(TyKind::Array(Box::new(items_ty.clone()))), span));
            Ok(items_ty)
        }
    }
}
