use indexmap::IndexMap;
use prqlc_ast::error::WithErrorInfo;
use prqlc_ast::{Ty, TyKind};

use crate::ir::decl::{Decl, DeclKind};
use crate::ir::pl;
use crate::{Error, Result};

#[derive(Debug)]
pub(super) struct Scope {
    pub generics: IndexMap<String, Decl>,

    pub params: IndexMap<String, Decl>,
}

pub enum LookupResult {
    Direct,
    Indirect {
        real_name: String,
        indirections: Vec<pl::IndirectionKind>,
    },
}

impl Scope {
    pub fn new() -> Self {
        Self {
            generics: IndexMap::new(),
            params: IndexMap::new(),
        }
    }

    pub fn get(&mut self, name: &str, only_types: bool) -> Option<&mut Decl> {
        if let Some(decl) = self.generics.get_mut(name) {
            return Some(decl);
        }
        if only_types {
            return None;
        }

        self.params.get_mut(name)
    }

    pub fn lookup(&self, name: &str, only_types: bool) -> Result<Option<LookupResult>> {
        if self.generics.contains_key(name) {
            return Ok(Some(LookupResult::Direct));
        }
        if only_types {
            return Ok(None);
        }

        if self.params.contains_key(name) {
            return Ok(Some(LookupResult::Direct));
        }

        let candidates = self.lookup_in_param_types(name, false);
        match candidates.len() {
            // no match: retry but also look into unpacks
            0 => {}

            // single match, great!
            1 => {
                let (param_name, indirections) = candidates.into_iter().next().unwrap();
                return Ok(Some(LookupResult::Indirect {
                    real_name: param_name.clone(),
                    indirections,
                }));
            }

            // ambiguous
            _ => return Err(ambiguous_error(candidates)),
        }

        let candidates = self.lookup_in_param_types(name, true);
        match candidates.len() {
            // no match: pass though
            0 => Err(Error::new_simple(format!("Unknown name `{name}`"))),

            // single match, great!
            1 => {
                let (param_name, indirections) = candidates.into_iter().next().unwrap();
                Ok(Some(LookupResult::Indirect {
                    real_name: param_name.clone(),
                    indirections,
                }))
            }

            // ambiguous
            _ => Err(ambiguous_error(candidates)),
        }
    }

    fn lookup_in_param_types(
        &self,
        name: &str,
        include_unpack: bool,
    ) -> Vec<(&String, Vec<pl::IndirectionKind>)> {
        let mut candidates = Vec::new();
        for (param_name, decl) in &self.params {
            let DeclKind::Variable(Some(var)) = &decl.kind else {
                continue;
            };

            for indirections in find_name_in_tuple(var, name, include_unpack) {
                candidates.push((param_name, indirections));
            }
        }
        candidates
    }
}

fn find_name_in_tuple<'a>(
    ty: &'a Ty,
    name: &'a str,
    include_unpack: bool,
) -> Vec<Vec<pl::IndirectionKind>> {
    let TyKind::Tuple(fields) = &ty.kind else {
        return vec![];
    };

    let mut res = Vec::new();
    for (pos, field) in fields.iter().enumerate() {
        match field {
            prqlc_ast::TyTupleField::Single(n, ty) => {
                if let Some(n) = n {
                    if n == name {
                        return vec![vec![pl::IndirectionKind::Name(n.clone())]];
                    }
                }
                if let Some(ty) = ty {
                    let indirection = if let Some(n) = n {
                        pl::IndirectionKind::Name(n.clone())
                    } else {
                        pl::IndirectionKind::Position(pos as i64)
                    };

                    for mut x in find_name_in_tuple(ty, name, include_unpack) {
                        x.insert(0, indirection.clone());
                        res.push(x);
                    }
                }
            }
            prqlc_ast::TyTupleField::Unpack(None) => {
                // TODO
            }
            prqlc_ast::TyTupleField::Unpack(Some(_)) => {
                if include_unpack {
                    // this assumes that ty is an ident to a generic arg.
                    // it *could* always be true, but i'm not sure atm
                    res.push(vec![pl::IndirectionKind::Name(name.to_string())]);
                }
            }
        }
    }
    res
}

fn ambiguous_error(candidates: Vec<(&String, Vec<pl::IndirectionKind>)>) -> Error {
    let mut candidates_str = Vec::new();
    for (first, indirections) in candidates {
        let mut r = first.clone();
        for indirection in indirections {
            r += ".";
            r += &match indirection {
                pl::IndirectionKind::Name(n) => n,
                pl::IndirectionKind::Position(pos) => pos.to_string(),
            };
        }
        candidates_str.push(r);
    }
    let hint = format!("could be any of: {}", candidates_str.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}
