use std::fmt::Display;

use chumsky::{error::SimpleReason, Span as ChumskySpan};
use prqlc_ast::Span;

use crate::{lexer::TokenKind, PError};

#[derive(Debug)]
pub struct Error {
    pub span: Span,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    Lexer(LexerError),
    Parser(ParserError),
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Lexer(err) => write!(f, "{err}"),
            ErrorKind::Parser(err) => write!(f, "{err}"),
        }
    }
}

#[derive(Debug)]
pub struct LexerError(String);

#[derive(Debug)]
pub struct ParserError(String);

impl Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unexpected {}", self.0)
    }
}

pub(crate) fn convert_lexer_error(
    source: &str,
    e: chumsky::error::Cheap<char>,
    source_id: u16,
) -> Error {
    // TODO: is there a neater way of taking a span? We want to take it based on
    // the chars, not the bytes, so can't just index into the str.
    let found = source
        .chars()
        .skip(e.span().start)
        .take(e.span().end() - e.span().start)
        .collect();
    let span = Span {
        start: e.span().start,
        end: e.span().end,
        source_id,
    };

    Error {
        span,
        kind: ErrorKind::Lexer(LexerError(found)),
    }
}

pub(crate) fn convert_parser_error(e: PError) -> Error {
    let mut span = e.span();

    if e.found().is_none() {
        // found end of file
        // fix for span outside of source
        if span.start > 0 && span.end > 0 {
            span.start -= 1;
            span.end -= 1;
        }
    }

    if let SimpleReason::Custom(message) = e.reason() {
        return Error {
            span: *span,
            kind: ErrorKind::Parser(ParserError(message.clone())),
        };
    }

    fn token_to_string(t: Option<TokenKind>) -> String {
        t.map(|t| DisplayToken(&t).to_string())
            .unwrap_or_else(|| "end of input".to_string())
    }

    let is_all_whitespace = e
        .expected()
        .all(|t| matches!(t, None | Some(TokenKind::NewLine)));
    let expected: Vec<String> = e
        .expected()
        // TODO: could we collapse this into a `filter_map`? (though semantically
        // identical)
        //
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

        return Error {
            span: *span,
            kind: ErrorKind::Parser(ParserError(format!("unexpected {label}{while_parsing}"))),
        };
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

    Error {
        span: *span,
        kind: ErrorKind::Parser(ParserError(match e.found() {
            Some(found) => format!(
                "{who}expected {expected}, but found {found}",
                who = e.label().map(|l| format!("{l} ")).unwrap_or_default(),
                found = DisplayToken(found)
            ),

            // We want a friendlier message than "found end of input"...
            None => format!("Expected {expected}, but didn't find anything before the end."),
        })),
    }
}

struct DisplayToken<'a>(&'a TokenKind);

impl std::fmt::Display for DisplayToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            TokenKind::NewLine => write!(f, "new line"),
            TokenKind::Ident(arg0) => {
                if arg0.is_empty() {
                    write!(f, "an identifier")
                } else {
                    write!(f, "`{arg0}`")
                }
            }
            TokenKind::Keyword(arg0) => write!(f, "keyword {arg0}"),
            TokenKind::Literal(..) => write!(f, "literal"),
            TokenKind::Control(arg0) => write!(f, "{arg0}"),

            TokenKind::ArrowThin => f.write_str("->"),
            TokenKind::ArrowFat => f.write_str("=>"),
            TokenKind::Eq => f.write_str("=="),
            TokenKind::Ne => f.write_str("!="),
            TokenKind::Gte => f.write_str(">="),
            TokenKind::Lte => f.write_str("<="),
            TokenKind::RegexSearch => f.write_str("~="),
            TokenKind::And => f.write_str("&&"),
            TokenKind::Or => f.write_str("||"),
            TokenKind::Coalesce => f.write_str("??"),
            TokenKind::DivInt => f.write_str("//"),
            TokenKind::Pow => f.write_str("**"),
            TokenKind::Annotate => f.write_str("@{"),

            TokenKind::Param(id) => write!(f, "${id}"),

            TokenKind::Range {
                bind_left,
                bind_right,
            } => write!(
                f,
                "'{}..{}'",
                if *bind_left { "" } else { " " },
                if *bind_right { "" } else { " " }
            ),
            TokenKind::Interpolation(c, s) => {
                write!(f, "{c}\"{}\"", s)
            }
        }
    }
}
