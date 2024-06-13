use super::{WriteOpt, WriteSource};
use crate::ast::*;
use crate::codegen::SeparatedExprs;

pub(crate) fn write_ty(ty: &Ty) -> String {
    ty.write(WriteOpt::new_width(u16::MAX)).unwrap()
}

pub(crate) fn write_ty_kind(ty: &TyKind) -> String {
    ty.write(WriteOpt::new_width(u16::MAX)).unwrap()
}

impl WriteSource for Ty {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        if let Some(name) = &self.name {
            Some(name.clone())
        } else {
            self.kind.write(opt)
        }
    }
}

impl WriteSource for Option<&Ty> {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Some(ty) => ty.write(opt),
            None => Some("infer".to_string()),
        }
    }
}

impl WriteSource for TyKind {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        use TyKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            Primitive(prim) => Some(prim.to_string()),
            Union(variants) => {
                let parenthesize =
                    // never must be parenthesized
                    variants.is_empty() ||
                    // named union must be parenthesized
                    variants.iter().any(|(n, _)| n.is_some());

                let variants: Vec<_> = variants.iter().map(|(n, t)| UnionVariant(n, t)).collect();
                let sep_exprs = SeparatedExprs {
                    exprs: &variants,
                    inline: " || ",
                    line_end: " ||",
                };

                if parenthesize {
                    sep_exprs.write_between("(", ")", opt)
                } else {
                    sep_exprs.write(opt)
                }
            }
            Singleton(lit) => Some(lit.to_string()),
            Tuple(elements) => SeparatedExprs {
                exprs: elements,
                inline: ", ",
                line_end: ",",
            }
            .write_between("{", "}", opt),
            Array(elem) => Some(format!("[{}]", elem.write(opt)?)),
            Function(None) => Some("func".to_string()),
            Function(Some(func)) => {
                let mut r = "func ".to_string();

                for t in &func.params {
                    r += &t.as_ref().write(opt.clone())?;
                    r += " ";
                }
                r += "-> ";
                r += &(*func.return_ty).as_ref().write(opt)?;
                Some(r)
            }
            Any => Some("anytype".to_string()),
            Difference { base, exclude } => {
                let base = base.write(opt.clone())?;
                let exclude = exclude.write(opt.clone())?;
                Some(format!("{base} - {exclude}"))
            }
            GenericArg(_) => Some("?".to_string()),
        }
    }
}

impl WriteSource for TyTupleField {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Self::Wildcard(generic_el) => match generic_el {
                Some(el) => Some(format!("{}..", el.write(opt)?)),
                None => Some("*..".to_string()),
            },
            Self::Single(name, expr) => {
                let mut r = String::new();

                if let Some(name) = name {
                    r += name;
                    r += " = ";
                }
                if let Some(expr) = expr {
                    r += &expr.write(opt)?;
                } else {
                    r += "?";
                }
                Some(r)
            }
        }
    }
}

struct UnionVariant<'a>(&'a Option<String>, &'a Ty);

impl WriteSource for UnionVariant<'_> {
    fn write(&self, mut opt: WriteOpt) -> Option<String> {
        let mut r = String::new();
        if let Some(name) = &self.0 {
            r += name;
            r += " = ";
        }
        opt.consume_width(r.len() as u16);
        r += &self.1.write(opt)?;
        Some(r)
    }
}
