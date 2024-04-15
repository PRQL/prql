use chumsky::prelude::*;

use prqlc_ast::*;

use crate::expr::ident;
use crate::span::ParserSpan;

use super::common::*;
use super::lexer::TokenKind;

pub fn type_expr() -> impl Parser<TokenKind, Ty, Error = PError> {
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
                    .then(nested_type_expr.clone().map(Some).map(Box::new))
                    .map(|(params, return_ty)| TyFunc {
                        params,
                        return_ty,
                        name_hint: None,
                    })
                    .or_not(),
            )
            .map(TyKind::Function);

        let tuple = choice((
            select! { TokenKind::Range { bind_right: true, bind_left: _ } => () }
                .ignore_then(nested_type_expr.clone())
                .map(|ty| TyTupleField::Unpack(Some(ty))),
            ident_part()
                .then_ignore(ctrl('='))
                .or_not()
                .then(nested_type_expr.clone())
                .map(|(name, ty)| TyTupleField::Single(name, Some(ty))),
        ))
        .padded_by(new_line().repeated())
        .separated_by(ctrl(','))
        .allow_trailing()
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

        // let union_parenthesized = ident_part()
        //     .then_ignore(ctrl('='))
        //     .or_not()
        //     .then(nested_type_expr.clone())
        //     .padded_by(new_line().repeated())
        //     .separated_by(just(TokenKind::Or))
        //     .allow_trailing()
        //     .then_ignore(new_line().repeated())
        //     .delimited_by(ctrl('('), ctrl(')'))
        //     .recover_with(nested_delimiters(
        //         TokenKind::Control('('),
        //         TokenKind::Control(')'),
        //         [
        //             (TokenKind::Control('{'), TokenKind::Control('}')),
        //             (TokenKind::Control('('), TokenKind::Control(')')),
        //             (TokenKind::Control('['), TokenKind::Control(']')),
        //         ],
        //         |_| vec![],
        //     ))
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
            .map_with_span(into_ty)
            .boxed();

        // // union
        // term.clone()
        //     .then(just(TokenKind::Or).ignore_then(term).repeated())
        //     .map_with_span(|(first, following), span| {
        //         if following.is_empty() {
        //             first
        //         } else {
        //             let mut all = Vec::with_capacity(following.len() + 1);
        //             all.push((None, first));
        //             all.extend(following.into_iter().map(|x| (None, x)));
        //             into_ty(TyKind::Union(all), span)
        //         }
        //     })

        // exclude
        term.clone()
            .then(ctrl('-').ignore_then(term).repeated())
            .foldl(|left, right| {
                let left_span = left.span.as_ref().unwrap();
                let right_span = right.span.as_ref().unwrap();
                let span = ParserSpan(Span {
                    start: left_span.start,
                    end: right_span.end,
                    source_id: left_span.source_id,
                });

                let kind = TyKind::Exclude {
                    base: Box::new(left),
                    except: Box::new(right),
                };
                into_ty(kind, span)
            })
    })
    .labelled("type expression")
}
