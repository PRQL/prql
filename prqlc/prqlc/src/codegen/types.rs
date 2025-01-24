use prqlc_parser::parser::pr;

use crate::codegen::SeparatedExprs;

use super::{WriteOpt, WriteSource};

pub(crate) fn write_ty(ty: &pr::Ty) -> String {
    ty.write(WriteOpt::new_width(u16::MAX)).unwrap()
}

pub(crate) fn write_ty_kind(ty: &pr::TyKind) -> String {
    ty.write(WriteOpt::new_width(u16::MAX)).unwrap()
}

impl WriteSource for pr::Ty {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        if let Some(name) = &self.name {
            Some(name.clone())
        } else {
            self.kind.write(opt)
        }
    }
}

impl WriteSource for Option<&pr::Ty> {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Some(ty) => ty.write(opt),
            None => Some("?".to_string()),
        }
    }
}

impl WriteSource for pr::TyKind {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        use pr::TyKind::*;

        match &self {
            Ident(ident) => ident.write(opt),
            Primitive(prim) => Some(prim.to_string()),
            Singleton(lit) => Some(lit.to_string()),
            Tuple(elements) => SeparatedExprs {
                exprs: elements,
                inline: ", ",
                line_end: ",",
            }
            .write_between("{", "}", opt),
            Array(Some(elem)) => Some(format!("[{}]", elem.write(opt)?)),
            Array(None) => Some("[]".into()),
            Function(None) => Some("func".to_string()),
            Function(Some(func)) => {
                let mut r = "func ".to_string();

                for t in &func.params {
                    r += &t.as_ref().write(opt.clone())?;
                    r += " ";
                }
                r += "-> ";
                r += &func.return_ty.as_deref().write(opt)?;
                Some(r)
            }
            Difference { base, exclude } => {
                let base = base.write(opt.clone())?;
                let exclude = exclude.write(opt.clone())?;
                Some(format!("{base} - {exclude}"))
            }
        }
    }
}

impl WriteSource for pr::TyTupleField {
    fn write(&self, opt: WriteOpt) -> Option<String> {
        match self {
            Self::Wildcard(generic_el) => match generic_el {
                Some(el) => Some(format!("{}..", el.write(opt)?)),
                None => Some("..".to_string()),
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
