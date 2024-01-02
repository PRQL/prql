use chumsky::prelude::*;

use prqlc_ast::*;

use crate::expr::ident;

use super::common::*;
use super::lexer::Token;

pub fn type_expr() -> impl Parser<Token, Ty, Error = PError> {
    recursive(|nested_type_expr| {
        let basic = select! {
            Token::Literal(lit) => TyKind::Singleton(lit),
            Token::Ident(i) if i == "int"=> TyKind::Primitive(PrimitiveSet::Int),
            Token::Ident(i) if i == "float"=> TyKind::Primitive(PrimitiveSet::Float),
            Token::Ident(i) if i == "bool"=> TyKind::Primitive(PrimitiveSet::Bool),
            Token::Ident(i) if i == "text"=> TyKind::Primitive(PrimitiveSet::Text),
            Token::Ident(i) if i == "date"=> TyKind::Primitive(PrimitiveSet::Date),
            Token::Ident(i) if i == "time"=> TyKind::Primitive(PrimitiveSet::Time),
            Token::Ident(i) if i == "timestamp"=> TyKind::Primitive(PrimitiveSet::Timestamp),
            Token::Ident(i) if i == "anytype"=> TyKind::Any,
        };

        let ident = ident().map(TyKind::Ident);

        let func = keyword("func")
            .ignore_then(
                nested_type_expr
                    .clone()
                    .map(Some)
                    .repeated()
                    .then_ignore(just(Token::ArrowThin))
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
                        Token::Range {
                            bind_left: true,
                            ..
                        }
                    )
                })
                .or_not(),
            )
            .map(|((name, ty), range)| {
                if range.is_some() {
                    TupleField::Wildcard(Some(ty))
                } else {
                    TupleField::Single(name, Some(ty))
                }
            })
            .padded_by(new_line().repeated())
            .separated_by(ctrl(','))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('{'), ctrl('}'))
            .recover_with(nested_delimiters(
                Token::Control('{'),
                Token::Control('}'),
                [
                    (Token::Control('{'), Token::Control('}')),
                    (Token::Control('('), Token::Control(')')),
                    (Token::Control('['), Token::Control(']')),
                ],
                |_| vec![],
            ))
            .map(TyKind::Tuple)
            .labelled("tuple");

        let union_parenthesized = ident_part()
            .then_ignore(ctrl('='))
            .or_not()
            .then(nested_type_expr.clone())
            .padded_by(new_line().repeated())
            .separated_by(just(Token::Or))
            .allow_trailing()
            .then_ignore(new_line().repeated())
            .delimited_by(ctrl('('), ctrl(')'))
            .recover_with(nested_delimiters(
                Token::Control('('),
                Token::Control(')'),
                [
                    (Token::Control('{'), Token::Control('}')),
                    (Token::Control('('), Token::Control(')')),
                    (Token::Control('['), Token::Control(']')),
                ],
                |_| vec![],
            ))
            .map(TyKind::Union)
            .labelled("union");

        let array = nested_type_expr
            .map(Box::new)
            .padded_by(new_line().repeated())
            .delimited_by(ctrl('['), ctrl(']'))
            .recover_with(nested_delimiters(
                Token::Control('['),
                Token::Control(']'),
                [
                    (Token::Control('{'), Token::Control('}')),
                    (Token::Control('('), Token::Control(')')),
                    (Token::Control('['), Token::Control(']')),
                ],
                |_| Box::new(Ty::new(Literal::Null)),
            ))
            .map(TyKind::Array)
            .labelled("array");

        let term = choice((basic, ident, func, tuple, array, union_parenthesized))
            .map_with_span(into_ty)
            .boxed();

        // union
        term.clone()
            .then(just(Token::Or).ignore_then(term).repeated())
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
