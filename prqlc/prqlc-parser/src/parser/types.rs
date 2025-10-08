use chumsky;
use chumsky::input::ValueInput;

use super::expr::ident;
use super::pr::*;
use super::*;
use crate::lexer::lr::TokenKind;

pub(crate) fn type_expr<'a, I>(
) -> impl Parser<'a, I, Ty, extra::Err<Rich<'a, TokenKind, Span>>> + Clone
where
    I: Input<'a, Token = TokenKind, Span = Span> + ValueInput<'a>,
{
    recursive(|nested_type_expr| {
        let basic = select! {
            TokenKind::Ident(i) if i == "int"=> TyKind::Primitive(PrimitiveSet::Int),
            TokenKind::Ident(i) if i == "float"=> TyKind::Primitive(PrimitiveSet::Float),
            TokenKind::Ident(i) if i == "bool"=> TyKind::Primitive(PrimitiveSet::Bool),
            TokenKind::Ident(i) if i == "text"=> TyKind::Primitive(PrimitiveSet::Text),
            TokenKind::Ident(i) if i == "date"=> TyKind::Primitive(PrimitiveSet::Date),
            TokenKind::Ident(i) if i == "time"=> TyKind::Primitive(PrimitiveSet::Time),
            TokenKind::Ident(i) if i == "timestamp"=> TyKind::Primitive(PrimitiveSet::Timestamp),
        };

        let ident = ident().map(TyKind::Ident);

        let func = keyword("func")
            .ignore_then(
                nested_type_expr
                    .clone()
                    .map(Some)
                    .repeated()
                    .collect()
                    .then_ignore(just(TokenKind::ArrowThin))
                    .then(nested_type_expr.clone().map(Box::new).map(Some))
                    .map(|(params, return_ty)| TyFunc {
                        name_hint: None,
                        params,
                        return_ty,
                    })
                    .or_not(),
            )
            .map(TyKind::Function);

        let tuple = sequence(choice((
            select! { TokenKind::Range { bind_right: false, bind_left: _ } => () }
                .to(TyTupleField::Wildcard(None)),
            select! { TokenKind::Range { bind_right: true, bind_left: _ } => () }
                .ignore_then(nested_type_expr.clone().or_not())
                .map(TyTupleField::Wildcard),
            ident_part()
                .then_ignore(ctrl('='))
                .or_not()
                .then(ctrl('*').to(None).or(nested_type_expr.clone().map(Some)))
                .map(|(name, ty)| TyTupleField::Single(name, ty)),
        )))
        .delimited_by(ctrl('{'), ctrl('}'))
        // TODO: Add back error recovery with Chumsky 0.10 API
        // .recover_with(...)
        .try_map(|fields, span| {
            let without_last = &fields[0..fields.len().saturating_sub(1)];

            if let Some(unpack) = without_last.iter().find_map(|f| f.as_wildcard()) {
                let err_span = unpack.as_ref().and_then(|s| s.span).unwrap_or(span);
                return Err(Rich::custom(
                    err_span,
                    "unpacking must come after all other fields",
                ));
            }

            Ok(fields)
        })
        .map(TyKind::Tuple)
        .labelled("tuple");

        let array = nested_type_expr
            .map(Box::new)
            .or_not()
            .padded_by(new_line().repeated())
            .delimited_by(ctrl('['), ctrl(']'))
            // TODO: Add back error recovery with Chumsky 0.10 API
            // .recover_with(...)
            .map(TyKind::Array)
            .labelled("array");

        choice((basic, ident, func, tuple, array))
            .map_with(|kind, extra| TyKind::into_ty(kind, extra.span()))

        // exclude
        // term.clone()
        //     .then(ctrl('-').ignore_then(term).repeated())
        //     .foldl(|left, right| {
        //         let left_span = left.span.as_ref().unwrap();
        //         let right_span = right.span.as_ref().unwrap();
        //         let span = Span {
        //             start: left_span.start,
        //             end: right_span.end,
        //             source_id: left_span.source_id,
        //         };

        //         let kind = TyKind::Exclude {
        //             base: Box::new(left),
        //             except: Box::new(right),
        //         };
        //         into_ty(kind, span)
        //     });
    })
    .labelled("type expression")
}
