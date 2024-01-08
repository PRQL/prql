mod expr;
mod interpolation;
mod lexer;
mod span;
mod stmt;
mod types;

use chumsky::error::SimpleReason;
use chumsky::{prelude::*, Stream};

use prqlc_ast::error::Error;
use prqlc_ast::error::Reason;
use prqlc_ast::stmt::*;
use prqlc_ast::Span;

use lexer::Token;
use lexer::{TokenSpan, TokenStream};
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

    let ast = if let Some(tokens) = tokens {
        let stream = prepare_stream(tokens, source, source_id);

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

pub fn lex_source(source: &str) -> Result<TokenStream, Vec<Error>> {
    lexer::lexer().parse(source).map(TokenStream).map_err(|e| {
        e.into_iter()
            .map(|x| convert_lexer_error(source, x, 0))
            .collect()
    })
}

mod common {
    use chumsky::prelude::*;
    use prqlc_ast::Ty;
    use prqlc_ast::TyKind;

    use super::{lexer::Token, span::ParserSpan};
    use prqlc_ast::expr::*;
    use prqlc_ast::stmt::*;

    pub type PError = Simple<Token, ParserSpan>;

    pub fn ident_part() -> impl Parser<Token, String, Error = PError> {
        select! { Token::Ident(ident) => ident }.map_err(|e: PError| {
            Simple::expected_input_found(
                e.span(),
                [Some(Token::Ident("".to_string()))],
                e.found().cloned(),
            )
        })
    }

    pub fn keyword(kw: &'static str) -> impl Parser<Token, (), Error = PError> + Clone {
        just(Token::Keyword(kw.to_string())).ignored()
    }

    pub fn new_line() -> impl Parser<Token, (), Error = PError> + Clone {
        just(Token::NewLine).ignored()
    }

    pub fn ctrl(char: char) -> impl Parser<Token, (), Error = PError> + Clone {
        just(Token::Control(char)).ignored()
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

fn prepare_stream(
    tokens: Vec<TokenSpan>,
    source: &str,
    source_id: u16,
) -> Stream<Token, ParserSpan, impl Iterator<Item = (Token, ParserSpan)> + Sized> {
    let tokens = tokens
        .into_iter()
        .map(move |TokenSpan(t, s)| (t, ParserSpan::new(source_id, s)));
    let len = source.chars().count();
    let eoi = ParserSpan(Span {
        start: len,
        end: len + 1,
        source_id,
    });
    Stream::from_iter(eoi, tokens)
}

fn convert_lexer_error(source: &str, e: chumsky::error::Cheap<char>, source_id: u16) -> Error {
    // TODO: is there a neater way of taking a span? We want to take it based on
    // the chars, not the bytes, so can't just index into the str.
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

    let mut e = Error::new(Reason::Unexpected { found });
    e.span = span;
    e
}

fn convert_parser_error(e: common::PError) -> Error {
    let mut span = e.span();

    if e.found().is_none() {
        // found end of file
        // fix for span outside of source
        if span.start > 0 && span.end > 0 {
            span.start -= 1;
            span.end -= 1;
        }
    }

    let mut e = construct_parser_error(e);
    e.span = Some(*span);
    e
}

fn construct_parser_error(e: Simple<Token, ParserSpan>) -> Error {
    if let SimpleReason::Custom(message) = e.reason() {
        return Error::new_simple(message);
    }

    fn token_to_string(t: Option<Token>) -> String {
        t.as_ref()
            .map(Token::to_string)
            .unwrap_or_else(|| "end of input".to_string())
    }

    let is_all_whitespace = e
        .expected()
        .all(|t| matches!(t, None | Some(Token::NewLine)));
    let expected: Vec<String> = e
        .expected()
        // Only include whitespace if we're _only_ expecting whitespace
        .filter(|t| is_all_whitespace || !matches!(t, None | Some(Token::NewLine)))
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
        TokenStream (
          0..1: Literal(Integer(5)),
          2..3: Control('+'),
          4..5: Literal(Integer(3)),
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
