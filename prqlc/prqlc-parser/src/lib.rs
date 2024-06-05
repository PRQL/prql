use chumsky::{prelude::*, Stream};
use err::error::Reason;
use err::error::{Error, WithErrorInfo};
use lexer::TokenKind;
pub use lexer::{Token, TokenVec};
use prqlc_ast::span::Span;
use prqlc_ast::stmt::*;

use crate::err::error::ErrorSource;

mod expr;
mod interpolation;
mod lexer;

mod stmt;
#[cfg(test)]
mod test;
mod types;

// pub use prqlc_ast;
pub mod err;

pub use prqlc_ast as ast;

/// Build PRQL AST from a PRQL query string.
pub fn parse_source(source: &str, source_id: u16) -> Result<Vec<Stmt>, Vec<Error>> {
    let mut errors = Vec::new();

    let (tokens, lex_errors) = ::chumsky::Parser::parse_recovery(&lexer::lexer(), source);

    log::debug!("Lex errors: {:?}", lex_errors);
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

        log::debug!("parse errors: {:?}", parse_errors);
        errors.extend(parse_errors.into_iter().map(|e| e.into()));

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
    use prqlc_ast::expr::*;
    use prqlc_ast::stmt::*;
    use prqlc_ast::Span;
    use prqlc_ast::Ty;
    use prqlc_ast::TyKind;

    use super::lexer::TokenKind;
    use crate::err::parse_error::PError;

    pub fn ident_part() -> impl Parser<TokenKind, String, Error = PError> {
        return select! {
            TokenKind::Ident(ident) => ident,
            TokenKind::Keyword(ident) if &ident == "module" => ident,
        }
        .map_err(|e: PError| {
            PError::expected_input_found(
                e.span(),
                [Some(TokenKind::Ident("".to_string()))],
                e.found().cloned(),
            )
        });
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

    pub fn into_stmt((annotations, kind): (Vec<Annotation>, StmtKind), span: Span) -> Stmt {
        Stmt {
            kind,
            span: Some(span),
            annotations,
        }
    }

    pub fn into_expr(kind: ExprKind, span: Span) -> Expr {
        Expr {
            span: Some(span),
            ..Expr::new(kind)
        }
    }

    pub fn into_ty(kind: TyKind, span: Span) -> Ty {
        Ty {
            span: Some(span),
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
) -> Stream<TokenKind, Span, impl Iterator<Item = (TokenKind, Span)> + Sized> {
    let tokens = tokens
        .into_iter()
        .map(move |token| (token.kind, Span::new(source_id, token.span)));
    let len = source.chars().count();
    let eoi = Span {
        start: len,
        end: len + 1,
        source_id,
    };
    Stream::from_iter(eoi, tokens)
}

#[test]
fn test_prepare_stream() {
    use insta::assert_yaml_snapshot;

    let input = "from artists | filter name == 'John'";
    let tokens = lex_source(input).unwrap();
    let mut stream = prepare_stream(tokens.0.into_iter(), input, 0);
    assert_yaml_snapshot!(stream.fetch_tokens().collect::<Vec<(TokenKind, Span)>>(), @r###"
    ---
    - - Ident: from
      - "0:0-4"
    - - Ident: artists
      - "0:5-12"
    - - Control: "|"
      - "0:13-14"
    - - Ident: filter
      - "0:15-21"
    - - Ident: name
      - "0:22-26"
    - - Eq
      - "0:27-29"
    - - Literal:
          String: John
      - "0:30-36"
    "###);
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

    Error::new(Reason::Unexpected { found })
        .with_span(span)
        .with_source(ErrorSource::Lexer(e))
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
