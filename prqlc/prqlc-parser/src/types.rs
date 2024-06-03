use chumsky::prelude::*;

use super::common::*;
use super::lexer::TokenKind;
use crate::ast::*;
use crate::err::parse_error::PError;
use crate::expr::ident;

pub fn type_expr() -> impl Parser<TokenKind, Ty, Error = PError> {
    recursive(|nested_type_expr| {
        let basic = select! {
            TokenKind::Literal(lit) => TyKind::Singleton(lit),
            TokenKind::Ident(i) if i == "int"=> TyKind::Primitive(PrimitiveSet::Int),
            TokenKind::Ident(i) if i == "float"=> TyKind::Primitive(PrimitiveSet::Float),
            TokenKind::Ident(i) if i == "bool"=> TyKind::Primitive(PrimitiveSet::Bool),
            TokenKind::Ident(i) if i == "text"=> TyKind::Primitive(PrimitiveSet::Text),
            TokenKind::Ident(i) if i == "date"=> TyKind::Primitive(PrimitiveSet::Date),
            TokenKind::Ident(i) if i == "time"=> TyKind::Primitive(PrimitiveSet::Time),
            TokenKind::Ident(i) if i == "timestamp"=> TyKind::Primitive(PrimitiveSet::Timestamp),
            TokenKind::Ident(i) if i == "anytype"=> TyKind::Any,
        };

        let ident = ident().map(TyKind::Ident);

        let func = keyword("func")
            .ignore_then(
                nested_type_expr
                    .clone()
                    .map(Some)
                    .repeated()
                    .then_ignore(just(TokenKind::ArrowThin))
                    .then(nested_type_expr.clone().map(Some).map(Box::new))
                    .map(|(args, return_ty)| TyFunc {
                        args,
                        return_ty,
                        name_hint: None,
                    })
                    .or_not(),
            )
            .map(TyKind::Function);

        let tuple = ident_part()
            .then_ignore(ctrl('='))
            .or_not()
            .then(nested_type_expr.clone())
            .then(
                filter(|x| {
                    matches!(
                        x,
                        TokenKind::Range {
                            bind_left: true,
                            ..
                        }
                    )
                })
                .or_not(),
            )
            .map(|((name, ty), range)| {
                if range.is_some() {
                    TyTupleField::Wildcard(Some(ty))
                } else {
                    TyTupleField::Single(name, Some(ty))
                }
            })
            .padded_by(new_line().repeated())
            .separated_by(ctrl(','))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('{'), ctrl('}'))
            .recover_with(nested_delimiters(
                TokenKind::Control('{'),
                TokenKind::Control('}'),
                [
                    (TokenKind::Control('{'), TokenKind::Control('}')),
                    (TokenKind::Control('('), TokenKind::Control(')')),
                    (TokenKind::Control('['), TokenKind::Control(']')),
                ],
                |_| vec![],
            ))
            .map(TyKind::Tuple)
            .labelled("tuple");

        let enum_ = keyword("enum")
            .ignore_then(
                ident_part()
                    .then(ctrl('=').ignore_then(nested_type_expr.clone()).or_not())
                    .map(|(name, ty)| {
                        (
                            Some(name),
                            ty.unwrap_or_else(|| Ty::new(TyKind::Tuple(vec![]))),
                        )
                    })
                    .padded_by(new_line().repeated())
                    .separated_by(ctrl(','))
                    .allow_trailing()
                    .then_ignore(new_line().repeated())
                    .delimited_by(ctrl('{'), ctrl('}'))
                    .recover_with(nested_delimiters(
                        TokenKind::Control('{'),
                        TokenKind::Control('}'),
                        [
                            (TokenKind::Control('{'), TokenKind::Control('}')),
                            (TokenKind::Control('('), TokenKind::Control(')')),
                            (TokenKind::Control('['), TokenKind::Control(']')),
                        ],
                        |_| vec![],
                    )),
            )
            .map(TyKind::Union)
            .labelled("union");

        let array = nested_type_expr
            .map(Box::new)
            .padded_by(new_line().repeated())
            .delimited_by(ctrl('['), ctrl(']'))
            .recover_with(nested_delimiters(
                TokenKind::Control('['),
                TokenKind::Control(']'),
                [
                    (TokenKind::Control('{'), TokenKind::Control('}')),
                    (TokenKind::Control('('), TokenKind::Control(')')),
                    (TokenKind::Control('['), TokenKind::Control(']')),
                ],
                |_| Box::new(Ty::new(Literal::Null)),
            ))
            .map(TyKind::Array)
            .labelled("array");

        let term = choice((basic, ident, func, tuple, array, enum_))
            .map_with_span(into_ty)
            .boxed();

        // union
        term.clone()
            .then(just(TokenKind::Or).ignore_then(term).repeated())
            .map_with_span(|(first, following), span| {
                if following.is_empty() {
                    first
                } else {
                    let mut all = Vec::with_capacity(following.len() + 1);
                    all.push((None, first));
                    all.extend(following.into_iter().map(|x| (None, x)));
                    into_ty(TyKind::Union(all), span)
                }
            })
    })
    .labelled("type expression")
}
