use std::borrow::Cow;

use itertools::Itertools;

use crate::pr::{Ident, Ty, TyKind, TyTupleField, PrimitiveSet};
use crate::codegen::write_ty;
use crate::ir::decl::DeclKind;
use crate::ir::pl::{Expr, ExprKind, IndirectionKind};
use crate::{Error, Result, WithErrorInfo};

// TODO: i'm not proud of the naming scheme in this file

pub fn lookup_position_in_tuple(base: &Ty, position: usize) -> Result<Option<StepOwned>> {
    // get base fields
    let TyKind::Tuple(fields) = &base.kind else {
        return Ok(None);
    };

    let unpack = fields.last().and_then(|f| match f {
        TyTupleField::Single(_, _) => None,
        TyTupleField::Unpack(Some(t)) => Some(t),
        TyTupleField::Unpack(None) => todo!(),
    });
    let singles = if unpack.is_some() {
        &fields[0..fields.len() - 1]
    } else {
        fields.as_slice()
    };

    Ok(if position < singles.len() {
        fields.get(position).map(|f| match f {
            TyTupleField::Single(_, Some(ty)) => StepOwned {
                position,
                target_ty: ty.clone(),
            },
            TyTupleField::Single(_, None) => todo!(),
            TyTupleField::Unpack(_) => unreachable!(),
        })
    } else if let Some(unpack) = unpack {
        let pos_here = singles.len();
        lookup_position_in_tuple(unpack, position - pos_here)?.map(|mut step| {
            step.position += pos_here;
            step
        })
    } else {
        None
    })
}

impl super::Resolver<'_> {
    /// Performs tuple indirection by name.
    pub fn lookup_name_in_tuple<'a>(
        &'a mut self,
        ty: &'a Ty,
        name: &str,
    ) -> Result<Option<Vec<StepOwned>>> {
        log::debug!("looking up `.{name}` in {}", write_ty(ty));

        // find existing field
        let found = self.find_name_in_tuple(ty, name);
        match found.len() {
            // no match: pass though
            0 => {}

            // single match, great!
            1 => {
                let found = found.into_iter().next().unwrap();
                return Ok(Some(
                    found.into_iter().map(|s| s.into_owned()).collect_vec(),
                ));
            }

            // ambiguous
            _ => return Err(ambiguous_error(found)),
        }

        // field was not found, find a generic where it could be added
        let generics = self.find_tuple_generic(ty, false);
        match generics.len() {
            // no match: pass though
            0 => {}

            // single match, great!
            1 => {
                let loc = generics.into_iter().next().unwrap();
                let pos_gen = loc.position;
                let ident_of_generic = loc.ident_of_generic.clone();

                let mut steps: Vec<StepOwned> = loc
                    .steps_to_base
                    .into_iter()
                    .map(|s| s.into_owned())
                    .collect();

                let indirection = IndirectionKind::Name(name.to_string());
                let (pos_within, target_ty) =
                    self.infer_generic_as_tuple(&ident_of_generic, indirection);

                steps.push(StepOwned {
                    position: pos_gen + pos_within,
                    target_ty,
                });
                return Ok(Some(steps));
            }

            // ambiguous
            _ => {
                let dummy = Ty::new(PrimitiveSet::Bool);
                let name = name.to_string();
                let candidates = generics
                    .into_iter()
                    .map(|mut loc| {
                        loc.steps_to_base.push(Step {
                            position: loc.position,
                            name: Some(&name),
                            target_ty: Cow::Borrowed(&dummy),
                        });
                        loc.steps_to_base
                    })
                    .collect();
                return Err(ambiguous_error(candidates));
            }
        }

        Ok(None)
    }

    fn get_tuple_or_generic_candidate<'a>(&'a self, ty: &'a Ty) -> &'a Ty {
        let TyKind::Ident(ident) = &ty.kind else {
            return ty;
        };
        let decl = self.get_ident(ident).unwrap();
        let DeclKind::GenericParam(Some((candidate, _))) = &decl.kind else {
            return ty;
        };

        candidate
    }

    /// Find in fields of this tuple (including the unpack)
    fn find_name_in_tuple<'a>(&'a self, ty: &'a Ty, name: &str) -> Vec<Vec<Step>> {
        let ty = self.get_tuple_or_generic_candidate(ty);

        let TyKind::Tuple(fields) = &ty.kind else {
            return vec![];
        };

        if let Some(step) = self.find_name_in_tuple_direct(ty, name) {
            return vec![vec![step]];
        };

        let mut res = vec![];
        for (position, field) in fields.iter().enumerate() {
            match field {
                TyTupleField::Single(n, Some(ty)) => {
                    for mut x in self.find_name_in_tuple(ty, name) {
                        x.insert(
                            0,
                            Step {
                                position,
                                name: n.as_ref(),
                                target_ty: Cow::Borrowed(ty),
                            },
                        );
                        res.push(x);
                    }
                }
                TyTupleField::Unpack(Some(unpack_ty)) => {
                    res.extend(self.find_name_in_tuple(unpack_ty, name));
                }
                TyTupleField::Single(_, None) => {
                    todo!()
                }
                TyTupleField::Unpack(None) => {
                    todo!()
                }
            }
        }
        res
    }

    /// Find in this tuple (including the unpack)
    fn find_name_in_tuple_direct<'a>(&'a self, ty: &'a Ty, name: &str) -> Option<Step<'a>> {
        let ty = self.get_tuple_or_generic_candidate(ty);

        let TyKind::Tuple(fields) = &ty.kind else {
            return None;
        };

        for (position, field) in fields.iter().enumerate() {
            match field {
                TyTupleField::Single(n, Some(ty)) => {
                    if n.as_ref().map_or(false, |n| n == name) {
                        return Some(Step {
                            position,
                            name: n.as_ref(),
                            target_ty: Cow::Borrowed(ty),
                        });
                    }
                }
                TyTupleField::Unpack(Some(unpack_ty)) => {
                    if let Some(mut step) = self.find_name_in_tuple_direct(unpack_ty, name) {
                        step.position += position;
                        return Some(step);
                    }
                }
                TyTupleField::Single(_, None) => todo!(),
                TyTupleField::Unpack(None) => todo!(),
            }
        }
        None
    }

    /// Utility function for wrapping an expression into additional indirections.
    /// For example, when we have `x.a`, but `x = {b = {a = int}}`, lookup will return steps `[b, a]`.
    /// This function converts `x` and `[b, a]` into `((x).b).a`.
    pub fn apply_indirections(&mut self, mut base: Expr, steps: Vec<StepOwned>) -> Expr {
        for step in steps {
            base = Expr {
                id: Some(self.id.gen()),
                ty: Some(step.target_ty),
                ..Expr::new(ExprKind::Indirection {
                    base: Box::new(base),
                    field: IndirectionKind::Position(step.position as i64),
                })
            }
        }
        base
    }

    /// Find identifier of the generic that must receive a new field,
    /// if we push a new name into this tuple.
    fn find_tuple_generic<'a>(
        &self,
        ty: &'a Ty,
        require_tuple: bool,
    ) -> Vec<LocationOfGeneric<'a>> {
        if let TyKind::Ident(ident_of_generic) = &ty.kind {
            if require_tuple {
                let Some(decl) = self.get_ident(ident_of_generic) else {
                    return vec![];
                };
                let Some((cand, _)) = decl.kind.as_generic_param().and_then(|p| p.as_ref()) else {
                    return vec![];
                };
                if !cand.kind.is_tuple() {
                    return vec![];
                }
            }

            return vec![LocationOfGeneric {
                ident_of_generic,
                position: 0,
                steps_to_base: vec![],
            }];
        };

        let TyKind::Tuple(fields) = &ty.kind else {
            return vec![];
        };

        if let Some(TyTupleField::Unpack(Some(unpack_ty))) = fields.last() {
            let mut found = self.find_tuple_generic(unpack_ty, require_tuple);
            if !found.is_empty() {
                for x in &mut found {
                    if x.steps_to_base.is_empty() {
                        x.position += fields.len() - 1;
                    } else {
                        x.steps_to_base.first_mut().unwrap().position += fields.len() - 1;
                    }
                }
                return found;
            }
        }

        let mut res = vec![];
        for (position, field) in fields.iter().enumerate() {
            if let TyTupleField::Single(n, Some(ty)) = field {
                for mut x in self.find_tuple_generic(ty, true) {
                    x.steps_to_base.insert(
                        0,
                        Step {
                            position,
                            name: n.as_ref(),
                            target_ty: Cow::Borrowed(ty),
                        },
                    );
                    res.push(x);
                }
            }
        }
        res
    }
}

#[derive(Debug, Clone)]
pub struct Step<'a> {
    position: usize,
    name: Option<&'a String>,
    target_ty: Cow<'a, Ty>,
}

impl<'a> Step<'a> {
    #[allow(dead_code)]
    fn into_indirection(self) -> IndirectionKind {
        if let Some(name) = self.name {
            IndirectionKind::Name(name.clone())
        } else {
            IndirectionKind::Position(self.position as i64)
        }
    }

    fn as_str(&self) -> Cow<str> {
        if let Some(name) = self.name {
            name.into()
        } else {
            self.position.to_string().into()
        }
    }

    fn into_owned(self) -> StepOwned {
        StepOwned {
            position: self.position,
            target_ty: self.target_ty.into_owned(),
        }
    }
}

#[derive(PartialEq)]
pub struct StepOwned {
    position: usize,
    target_ty: Ty,
}

impl std::fmt::Debug for StepOwned {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepOwned")
            .field("position", &self.position)
            .field("target_ty", &write_ty(&self.target_ty))
            .finish()
    }
}

struct LocationOfGeneric<'a> {
    ident_of_generic: &'a Ident,
    position: usize,
    steps_to_base: Vec<Step<'a>>,
}

fn ambiguous_error(candidates: Vec<Vec<Step>>) -> Error {
    let mut candidates_str = Vec::new();
    for steps in candidates {
        let mut steps = steps.into_iter();

        let first = steps.next().unwrap();
        let mut r = first.as_str().to_string();
        for step in steps {
            r += ".";
            r += &step.as_str();
        }
        candidates_str.push(r);
    }
    let hint = format!("could be any of: {}", candidates_str.join(", "));
    Error::new_simple("Ambiguous name").push_hint(hint)
}

#[cfg(test)]
mod test {
    use crate::pr::Ty;
    use crate::ir::decl::RootModule;
    use crate::parser::parse;
    use crate::semantic::resolver::tuple::StepOwned;
    use crate::semantic::resolver::Resolver;
    use crate::{Error, Result, SourceTree};

    fn parse_ty(source: &str) -> Ty {
        let s = SourceTree::from(format!("type X = {source}"));
        let mod_def = parse(&s).unwrap();
        let stmt = mod_def.stmts.into_iter().next().unwrap();
        let ty_def = stmt.kind.into_type_def().unwrap();

        ty_def.value.unwrap()
    }

    fn tuple_lookup(tuple: &str, name: &str) -> Result<Vec<StepOwned>> {
        let mut root_module = RootModule::default();
        let mut r = Resolver::new(&mut root_module);

        r.lookup_name_in_tuple(&parse_ty(tuple), name)
            .and_then(|x| match x {
                Some(x) => Ok(x),
                None => Err(Error::new_simple("unknown name")),
            })
    }

    fn tuple_lookup_with_generic(tuple: &str, name: &str) -> Result<(Vec<StepOwned>, Ty)> {
        let mut root_module = RootModule::default();
        let mut r = Resolver::new(&mut root_module);

        // generate a new generic type (tests expect it to get name 'X1')
        let ident = r.init_new_global_generic("X");
        assert_eq!(ident.to_string(), "_generic.X1");

        // do the lookup
        let res = r.lookup_name_in_tuple(&parse_ty(tuple), name)?.unwrap();

        // get the generic candidate that was inferred
        let decl = r.get_ident(&ident).unwrap();
        let generic = decl.kind.as_generic_param().unwrap();
        let generic_candidate = generic.clone().unwrap().0;
        Ok((res, generic_candidate))
    }

    // ```prql
    // let x = {
    //   a = 1,
    //   b = {
    //     a = 2,
    //     b = 3,
    //     c = 4,
    //     d = {
    //       e = 5,
    //       ..G101
    //     }
    //   },
    //   c = 3,
    //   ..G100
    // }
    //
    // let y5 = x.f   # G102 (indirections: .3)
    // let y6 = x.b.f # G103 (indirections: .1.3.1)

    #[test]
    fn simple() {
        assert_eq!(
            tuple_lookup("{a = int}", "a").unwrap(),
            vec![StepOwned {
                position: 0,
                target_ty: parse_ty("int")
            }]
        );

        assert_eq!(
            tuple_lookup("{a = int, int, b = bool}", "b").unwrap(),
            vec![StepOwned {
                position: 2,
                target_ty: parse_ty("bool")
            }]
        );
    }

    #[test]
    fn unpack() {
        assert_eq!(
            tuple_lookup("{a = int, ..{b = bool}}", "b").unwrap(),
            vec![StepOwned {
                position: 1,
                target_ty: parse_ty("bool")
            }]
        );

        assert_eq!(
            tuple_lookup(
                "{a = int, ..{b = bool, ..{c = int, bool, ..{d = bool}}}}",
                "d"
            )
            .unwrap(),
            vec![StepOwned {
                position: 4,
                target_ty: parse_ty("bool")
            }]
        );
    }

    #[test]
    fn nested() {
        assert_eq!(
            tuple_lookup("{a = int, b = {bool, bool, c = int}}", "c").unwrap(),
            vec![
                StepOwned {
                    position: 1,
                    target_ty: parse_ty("{bool, bool, c = int}")
                },
                StepOwned {
                    position: 2,
                    target_ty: parse_ty("int")
                }
            ]
        );

        assert_eq!(
            tuple_lookup("{a = int, {b = int, {{c = int}, d = bool}, e = bool}}", "c").unwrap(),
            vec![
                StepOwned {
                    position: 1,
                    target_ty: parse_ty("{b = int, {{c = int}, d = bool}, e = bool}")
                },
                StepOwned {
                    position: 1,
                    target_ty: parse_ty("{{c = int}, d = bool}")
                },
                StepOwned {
                    position: 0,
                    target_ty: parse_ty("{c = int}")
                },
                StepOwned {
                    position: 0,
                    target_ty: parse_ty("int")
                },
            ]
        );

        // ambiguous
        tuple_lookup("{a = {c = int}, b = {c = int}}", "c").unwrap_err();

        // ambiguous
        tuple_lookup("{{c = int}, {c = int}}", "c").unwrap_err();

        // ambiguous
        tuple_lookup("{a = {c = int}, ..{b = {c = int}}}", "c").unwrap_err();

        assert_eq!(
            tuple_lookup("{a = int, b = {a = int}}", "a").unwrap(),
            vec![StepOwned {
                position: 0,
                target_ty: parse_ty("int")
            }]
        );

        assert_eq!(
            tuple_lookup("{a = int, b = {a = int}}", "a").unwrap(),
            vec![StepOwned {
                position: 0,
                target_ty: parse_ty("int")
            }]
        );
    }

    #[test]
    fn generic() {
        assert_eq!(
            tuple_lookup_with_generic("{a = int, .._generic.X1}", "b").unwrap(),
            (
                vec![StepOwned {
                    position: 1,
                    target_ty: parse_ty("_generic.F2")
                }],
                parse_ty("{b = _generic.F2}")
            )
        );

        assert_eq!(
            tuple_lookup_with_generic(
                "{a = int, b = {c = int, .._generic.X1}, .._generic.X1}",
                "d"
            )
            .unwrap(),
            (
                vec![StepOwned {
                    position: 2,
                    target_ty: parse_ty("_generic.F2")
                }],
                parse_ty("{d = _generic.F2}")
            )
        );

        assert_eq!(
            tuple_lookup_with_generic(
                "{a = int, b = {c = int, .._generic.X1}, ..{c = int, .._generic.X1}}",
                "d"
            )
            .unwrap(),
            (
                vec![StepOwned {
                    position: 3,
                    target_ty: parse_ty("_generic.F2")
                }],
                parse_ty("{d = _generic.F2}")
            )
        );

        assert_eq!(
            tuple_lookup_with_generic("{a = int, b = {c = int, .._generic.X1}, ..{c = int}}", "d")
                .unwrap(),
            (
                vec![
                    StepOwned {
                        position: 1,
                        target_ty: parse_ty("{c = int, .._generic.X1}")
                    },
                    StepOwned {
                        position: 1,
                        target_ty: parse_ty("_generic.F2")
                    }
                ],
                parse_ty("{d = _generic.F2}")
            )
        );

        assert_eq!(
            tuple_lookup_with_generic("{a = _generic.X1, .._generic.X1}", "b").unwrap(),
            (
                vec![
                    StepOwned {
                        position: 0,
                        target_ty: parse_ty("_generic.X1")
                    },
                    StepOwned {
                        position: 0,
                        target_ty: parse_ty("_generic.F2")
                    }
                ],
                parse_ty("{b = _generic.F2}")
            )
        );
    }
}
