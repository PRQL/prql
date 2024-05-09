mod expr;
mod interpolation;
mod lexer;
mod span;
mod stmt;
#[cfg(test)]
mod test;
mod types;

use chumsky::error::SimpleReason;
use chumsky::{prelude::*, Stream};

use prqlc_ast::error::Reason;
use prqlc_ast::error::{Error, WithErrorInfo};
use prqlc_ast::stmt::*;
use prqlc_ast::Span;

pub use lexer::Token;
pub use lexer::TokenKind;
pub use lexer::TokenVec;
use span::ParserSpan;

/// Build PRQL AST from a PRQL query string.
pub fn parse_source(source: &str, source_id: u16) -> Result<Vec<Stmt>, Vec<Error>> {
    let mut errors = Vec::new();

    let (tokens, lex_errors) = ::chumsky::Parser::parse_recovery(&lexer::lexer(), source);

    errors.extend(
        lex_errors
            .into_iter()
            .map(|e| convert_lexer_error(source, e, source_id)),
    );

    // We don't want comments in the AST (but we do intend to use them as part of
    // formatting)
    let semantic_tokens: Option<_> = tokens.map(|tokens| {
        tokens.into_iter().filter(|token| {
            !matches!(
                token.kind,
                TokenKind::Comment(_) | TokenKind::LineWrap(_) | TokenKind::DocComment(_)
            )
        })
    });

    let ast = if let Some(semantic_tokens) = semantic_tokens {
        let stream = prepare_stream(semantic_tokens, source, source_id);

        let (ast, parse_errors) = ::chumsky::Parser::parse_recovery(&stmt::source(), stream);

        errors.extend(parse_errors.into_iter().map(convert_parser_error));

        ast
    } else {
        None
    };

    if errors.is_empty() {
        Ok(ast.unwrap_or_default())
    } else {
        Err(errors)
    }
}

pub fn lex_source(source: &str) -> Result<TokenVec, Vec<Error>> {
    lexer::lexer().parse(source).map(TokenVec).map_err(|e| {
        e.into_iter()
            .map(|x| convert_lexer_error(source, x, 0))
            .collect()
    })
}

mod common {
    use chumsky::prelude::*;
    use prqlc_ast::Ty;
    use prqlc_ast::TyKind;

    use super::{lexer::TokenKind, span::ParserSpan};
    use prqlc_ast::expr::*;
    use prqlc_ast::stmt::*;

    pub type PError = Simple<TokenKind, ParserSpan>;

    pub fn ident_part() -> impl Parser<TokenKind, String, Error = PError> {
        select! {
            TokenKind::Ident(ident) => ident,
            TokenKind::Keyword(ident) if &ident == "module" => ident,
        }
        .map_err(|e: PError| {
            Simple::expected_input_found(
                e.span(),
                [Some(TokenKind::Ident("".to_string()))],
                e.found().cloned(),
            )
        })
    }

    pub fn keyword(kw: &'static str) -> impl Parser<TokenKind, (), Error = PError> + Clone {
        just(TokenKind::Keyword(kw.to_string())).ignored()
    }

    pub fn new_line() -> impl Parser<TokenKind, (), Error = PError> + Clone {
        just(TokenKind::NewLine).ignored()
    }

    pub fn ctrl(char: char) -> impl Parser<TokenKind, (), Error = PError> + Clone {
        just(TokenKind::Control(char)).ignored()
    }

    pub fn into_stmt((annotations, kind): (Vec<Annotation>, StmtKind), span: ParserSpan) -> Stmt {
        Stmt {
            kind,
            span: Some(span.0),
            annotations,
        }
    }

    pub fn into_expr(kind: ExprKind, span: ParserSpan) -> Expr {
        Expr {
            span: Some(span.0),
            ..Expr::new(kind)
        }
    }

    pub fn into_ty(kind: TyKind, span: ParserSpan) -> Ty {
        Ty {
            span: Some(span.0),
            ..Ty::new(kind)
        }
    }
}

/// Convert the output of the lexer into the input of the parser. Requires
/// supplying the original source code.
fn prepare_stream(
    tokens: impl Iterator<Item = Token>,
    source: &str,
    source_id: u16,
) -> Stream<TokenKind, ParserSpan, impl Iterator<Item = (TokenKind, ParserSpan)> + Sized> {
    let tokens = tokens
        .into_iter()
        .map(move |token| (token.kind, ParserSpan::new(source_id, token.span)));
    let len = source.chars().count();
    let eoi = ParserSpan(Span {
        start: len,
        end: len + 1,
        source_id,
    });
    Stream::from_iter(eoi, tokens)
}

fn convert_lexer_error(source: &str, e: chumsky::error::Cheap<char>, source_id: u16) -> Error {
    // We want to slice based on the chars, not the bytes, so can't just index
    // into the str.
    let found = source
        .chars()
        .skip(e.span().start)
        .take(e.span().end() - e.span().start)
        .collect();
    let span = Some(Span {
        start: e.span().start,
        end: e.span().end,
        source_id,
    });

    Error::new(Reason::Unexpected { found }).with_span(span)
}

fn convert_parser_error(e: common::PError) -> Error {
    let mut span = e.span();

    if e.found().is_none() {
        // found end of file
        // fix for span outside of source
        if span.start > 0 && span.end > 0 {
            span = span - 1;
        }
    }

    construct_parser_error(e).with_span(Some(span.0))
}

fn construct_parser_error(e: Simple<TokenKind, ParserSpan>) -> Error {
    if let SimpleReason::Custom(message) = e.reason() {
        return Error::new_simple(message);
    }

    fn token_to_string(t: Option<TokenKind>) -> String {
        t.as_ref()
            .map(TokenKind::to_string)
            .unwrap_or_else(|| "end of input".to_string())
    }

    let is_all_whitespace = e
        .expected()
        .all(|t| matches!(t, None | Some(TokenKind::NewLine)));
    let expected: Vec<String> = e
        .expected()
        // Only include whitespace if we're _only_ expecting whitespace
        .filter(|t| is_all_whitespace || !matches!(t, None | Some(TokenKind::NewLine)))
        .cloned()
        .map(token_to_string)
        .collect();

    let while_parsing = e
        .label()
        .map(|l| format!(" while parsing {l}"))
        .unwrap_or_default();

    if expected.is_empty() || expected.len() > 10 {
        let label = token_to_string(e.found().cloned());
        return Error::new_simple(format!("unexpected {label}{while_parsing}"));
    }

    let mut expected = expected;
    expected.sort();

    let expected = match expected.len() {
        1 => expected.remove(0),
        2 => expected.join(" or "),
        _ => {
            let last = expected.pop().unwrap();
            format!("one of {} or {last}", expected.join(", "))
        }
    };

    match e.found() {
        Some(found) => Error::new(Reason::Expected {
            who: e.label().map(|x| x.to_string()),
            expected,
            found: found.to_string(),
        }),
        // We want a friendlier message than "found end of input"...
        None => Error::new(Reason::Simple(format!(
            "Expected {expected}, but didn't find anything before the end."
        ))),
    }
}

#[test]
fn test_lex_source() {
    use insta::assert_debug_snapshot;

    assert_debug_snapshot!(lex_source("5 + 3"), @r###"
    Ok(
        TokenVec(
            [
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                4..5: Literal(Integer(3)),
            ],
        ),
    )
    "###);

    // Something that will generate an error
    assert_debug_snapshot!(lex_source("^"), @r###"
    Err(
        [
            Error {
                kind: Error,
                span: Some(
                    0:0-1,
                ),
                reason: Unexpected {
                    found: "^",
                },
                hints: [],
                code: None,
            },
        ],
    )
    "###);
}
