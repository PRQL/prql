use chumsky::prelude::*;

use super::expr::ident;
use super::perror::PError;
use super::pr::*;
use super::*;
use crate::lexer::lr::TokenKind;

pub(crate) fn type_expr() -> impl Parser<TokenKind, Ty, Error = PError> + Clone {
    recursive(|nested_type_expr| {
        let basic = select! {
            // TokenKind::Literal(lit) => TyKind::Singleton(lit),
            TokenKind::Ident(i) if i == "int"=> TyKind::Primitive(PrimitiveSet::Int),
            TokenKind::Ident(i) if i == "float"=> TyKind::Primitive(PrimitiveSet::Float),
            TokenKind::Ident(i) if i == "bool"=> TyKind::Primitive(PrimitiveSet::Bool),
            TokenKind::Ident(i) if i == "text"=> TyKind::Primitive(PrimitiveSet::Text),
            TokenKind::Ident(i) if i == "date"=> TyKind::Primitive(PrimitiveSet::Date),
            TokenKind::Ident(i) if i == "time"=> TyKind::Primitive(PrimitiveSet::Time),
            TokenKind::Ident(i) if i == "timestamp"=> TyKind::Primitive(PrimitiveSet::Timestamp),
            // TokenKind::Ident(i) if i == "anytype"=> TyKind::Any,
        };

        let ident = ident().map(TyKind::Ident);

        let func = keyword("func")
            .ignore_then(
                nested_type_expr
                    .clone()
                    .map(Some)
                    .repeated()
                    .then_ignore(just(TokenKind::ArrowThin))
                    .then(nested_type_expr.clone().map(Box::new).map(Some))
                    .map(|(params, return_ty)| TyFunc {
                        params,
                        return_ty,
                        generic_type_params: vec![],
                    })
                    .or_not(),
            )
            .map(TyKind::Function);

        let tuple = sequence(choice((
            select! { TokenKind::Range { bind_right: true, bind_left: _ } => () }
                .ignore_then(nested_type_expr.clone())
                .map(|ty| TyTupleField::Unpack(Some(ty))),
            ident_part()
                .then_ignore(ctrl('='))
                .or_not()
                .then(nested_type_expr.clone())
                .map(|(name, ty)| TyTupleField::Single(name, Some(ty))),
        )))
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
        .try_map(|fields, span| {
            let without_last = &fields[0..fields.len().saturating_sub(1)];

            if let Some(unpack) = without_last.iter().find_map(|f| f.as_unpack()) {
                let span = unpack.as_ref().and_then(|s| s.span).unwrap_or(span);
                return Err(PError::custom(
                    span,
                    "unpacking must come after all other fields",
                ));
            }

            Ok(fields)
        })
        .map(TyKind::Tuple)
        .labelled("tuple");

        // let enum_ = keyword("enum")
        //     .ignore_then(
        //         sequence(
        //             ident_part()
        //                 .then(ctrl('=').ignore_then(nested_type_expr.clone()).or_not())
        //                 .map(|(name, ty)| {
        //                     (
        //                         Some(name),
        //                         ty.unwrap_or_else(|| Ty::new(TyKind::Tuple(vec![]))),
        //                     )
        //                 }),
        //         )
        //         .delimited_by(ctrl('{'), ctrl('}'))
        //         .recover_with(nested_delimiters(
        //             TokenKind::Control('{'),
        //             TokenKind::Control('}'),
        //             [
        //                 (TokenKind::Control('{'), TokenKind::Control('}')),
        //                 (TokenKind::Control('('), TokenKind::Control(')')),
        //                 (TokenKind::Control('['), TokenKind::Control(']')),
        //             ],
        //             |_| vec![],
        //         )),
        //     )
        //     .map(TyKind::Union)
        //     .labelled("union");

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
                |_| Box::new(Ty::new(TyKind::Tuple(vec![]))),
            ))
            .map(TyKind::Array)
            .labelled("array");

        let term = choice((basic, ident, func, tuple, array))
            .map_with_span(TyKind::into_ty)
            .boxed();

        // exclude
        term.clone()
            .then(ctrl('-').ignore_then(term).repeated())
            .foldl(|left, right| {
                let left_span = left.span.as_ref().unwrap();
                let right_span = right.span.as_ref().unwrap();
                let span = Span {
                    start: left_span.start,
                    end: right_span.end,
                    source_id: left_span.source_id,
                };

                let kind = TyKind::Exclude {
                    base: Box::new(left),
                    except: Box::new(right),
                };
                TyKind::into_ty(kind, span)
            })
    })
    .labelled("type expression")
}
