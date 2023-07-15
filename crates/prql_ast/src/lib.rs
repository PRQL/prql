pub mod codegen;
pub mod expr;
pub mod fold;
mod ident;
pub mod literal;
pub mod stmt;

pub use ident::{is_valid_ident, Ident};

// Serialize the Other variant as untagged. This custom Serialize impl is only temporary ... so that the snapshot tests still pass.
#[rustfmt::skip]
impl<T: expr::Extension> serde::Serialize for expr::ExprKind<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            expr::ExprKind::Ident(value) => serializer.serialize_newtype_variant("ExprKind", 0, "Ident", value),
            expr::ExprKind::Literal(value) => serializer.serialize_newtype_variant("ExprKind", 1, "Literal", value),
            expr::ExprKind::Pipeline(value) => serializer.serialize_newtype_variant("ExprKind", 2, "Pipeline", value),
            expr::ExprKind::Tuple(value) => serializer.serialize_newtype_variant("ExprKind", 3, "Tuple", value),
            expr::ExprKind::Array(value) => serializer.serialize_newtype_variant("ExprKind", 4, "Array", value),
            expr::ExprKind::Range(value) => serializer.serialize_newtype_variant("ExprKind", 5, "Range", value),
            expr::ExprKind::Binary(value) => serializer.serialize_newtype_variant("ExprKind", 6, "Binary", value),
            expr::ExprKind::Unary(value) => serializer.serialize_newtype_variant("ExprKind", 7, "Unary", value),
            expr::ExprKind::FuncCall(value) => serializer.serialize_newtype_variant("ExprKind", 8, "FuncCall", value),
            expr::ExprKind::Func(value) => serializer.serialize_newtype_variant("ExprKind", 9, "Func", value),
            expr::ExprKind::SString(value) => serializer.serialize_newtype_variant("ExprKind", 10, "SString", value),
            expr::ExprKind::FString(value) => serializer.serialize_newtype_variant("ExprKind", 11, "FString", value),
            expr::ExprKind::Case(value) => serializer.serialize_newtype_variant("ExprKind", 12, "Case", value),
            expr::ExprKind::Param(value) => serializer.serialize_newtype_variant("ExprKind", 13, "Param", value),
            expr::ExprKind::Internal(value) => serializer.serialize_newtype_variant("ExprKind", 14, "Internal", value),
            expr::ExprKind::Other(value) => value.serialize(serializer),
        }
    }
}
