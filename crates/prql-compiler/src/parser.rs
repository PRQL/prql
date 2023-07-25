use std::collections::HashMap;

use anyhow::Result;
use chumsky::{error::SimpleReason, Span as ChumskySpan};
use prql_ast::{stmt::Stmt, Span};
use prql_parser::chumsky;

use crate::error::{Error, Errors, Reason, WithErrorInfo};
use crate::utils::IdGenerator;
use crate::{codegen, SourceTree};
use prql_parser::lexer::Token;

pub fn parse(file_tree: &SourceTree<String>) -> Result<SourceTree<Vec<Stmt>>> {
    let mut res = SourceTree::default();

    let ids: HashMap<_, _> = file_tree.source_ids.iter().map(|(a, b)| (b, a)).collect();
    let mut id_gen = IdGenerator::<usize>::new();

    for (path, source) in &file_tree.sources {
        let id = ids
            .get(path)
            .map(|x| **x)
            .unwrap_or_else(|| id_gen.gen() as u16);
        let stmts = parse_source(source, id)?;

        res.sources.insert(path.clone(), stmts);
        res.source_ids.insert(id, path.clone());
    }
    Ok(res)
}

fn parse_source(source: &str, source_id: u16) -> Result<Vec<prql_ast::stmt::Stmt>> {
    let stmts = prql_parser::parse_source(source, source_id).map_err(|errors| {
        Errors(
            errors
                .into_iter()
                .map(|err| match err {
                    prql_parser::Error::Lexer(err) => convert_lexer_error(source, err, source_id),
                    prql_parser::Error::Parser(err) => convert_parser_error(err),
                })
                .collect(),
        )
    })?;

    Ok(stmts)
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

    Error::new(Reason::Unexpected { found }).with_span(span)
}

fn convert_parser_error(e: prql_parser::PError) -> Error {
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
        return Error::new_simple(message).with_span(Some(*span));
    }

    fn token_to_string(t: Option<Token>) -> String {
        t.map(|t| DisplayToken(&t).to_string())
            .unwrap_or_else(|| "end of input".to_string())
    }

    let is_all_whitespace = e
        .expected()
        .all(|t| matches!(t, None | Some(Token::NewLine)));
    let expected: Vec<String> = e
        .expected()
        // TODO: could we collapse this into a `filter_map`? (though semantically
        // identical)
        //
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
        return Error::new_simple(format!("unexpected {label}{while_parsing}"))
            .with_span(Some(*span));
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
            found: DisplayToken(found).to_string(),
        }),
        // We want a friendlier message than "found end of input"...
        None => Error::new(Reason::Simple(format!(
            "Expected {expected}, but didn't find anything before the end."
        ))),
    }
    .with_span(Some(*span))
}

struct DisplayToken<'a>(&'a Token);

impl std::fmt::Display for DisplayToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Token::NewLine => write!(f, "new line"),
            Token::Ident(arg0) => {
                if arg0.is_empty() {
                    write!(f, "an identifier")
                } else {
                    write!(f, "`{arg0}`")
                }
            }
            Token::Keyword(arg0) => write!(f, "keyword {arg0}"),
            Token::Literal(arg0) => write!(f, "{}", codegen::DisplayLiteral(arg0)),
            Token::Control(arg0) => write!(f, "{arg0}"),

            Token::ArrowThin => f.write_str("->"),
            Token::ArrowFat => f.write_str("=>"),
            Token::Eq => f.write_str("=="),
            Token::Ne => f.write_str("!="),
            Token::Gte => f.write_str(">="),
            Token::Lte => f.write_str("<="),
            Token::RegexSearch => f.write_str("~="),
            Token::And => f.write_str("&&"),
            Token::Or => f.write_str("||"),
            Token::Coalesce => f.write_str("??"),
            Token::DivInt => f.write_str("//"),
            Token::Annotate => f.write_str("@{"),

            Token::Param(id) => write!(f, "${id}"),

            Token::Range {
                bind_left,
                bind_right,
            } => write!(
                f,
                "'{}..{}'",
                if *bind_left { "" } else { " " },
                if *bind_right { "" } else { " " }
            ),
            Token::Interpolation(c, s) => {
                write!(f, "{c}\"{}\"", s)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use prql_ast::stmt::Stmt;

    /// Helper that does not track source_ids
    #[cfg(test)]
    pub fn parse_single(source: &str) -> anyhow::Result<Vec<Stmt>> {
        super::parse_source(source, 0)
    }

    #[test]
    fn test_error_unicode_string() {
        // Test various unicode strings successfully parse errors. We were
        // getting loops in the lexer before.
        parse_single("s’ ").unwrap_err();
        parse_single("s’").unwrap_err();
        parse_single(" s’").unwrap_err();
        parse_single(" ’ s").unwrap_err();
        parse_single("’s").unwrap_err();
        parse_single("👍 s’").unwrap_err();

        let source = "Mississippi has four S’s and four I’s.";
        assert_debug_snapshot!(parse_single(source).unwrap_err(), @r###"
        Errors(
            [
                Error {
                    kind: Error,
                    span: Some(
                        0:22-23,
                    ),
                    reason: Unexpected {
                        found: "’",
                    },
                    hints: [],
                    code: None,
                },
                Error {
                    kind: Error,
                    span: Some(
                        0:35-36,
                    ),
                    reason: Unexpected {
                        found: "’",
                    },
                    hints: [],
                    code: None,
                },
                Error {
                    kind: Error,
                    span: Some(
                        0:37-38,
                    ),
                    reason: Simple(
                        "Expected * or an identifier, but didn't find anything before the end.",
                    ),
                    hints: [],
                    code: None,
                },
            ],
        )
        "###);
    }

    #[test]
    fn test_error_unexpected() {
        assert_debug_snapshot!(parse_single("Answer: T-H-A-T!").unwrap_err(), @r###"
        Errors(
            [
                Error {
                    kind: Error,
                    span: Some(
                        0:6-7,
                    ),
                    reason: Simple(
                        "unexpected : while parsing source file",
                    ),
                    hints: [],
                    code: None,
                },
            ],
        )
        "###);
    }
}
