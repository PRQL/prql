/*
# Implementation Plan for Chumsky 0.10.0 Lexer

## Setup
- ✅ Create feature flag structure
- ✅ Set up parallel module for 0.10 implementation
- ✅ Create stub functions for the new lexer

## Resources

Check out these issues for more details:
- https://github.com/zesterer/chumsky/issues/747
- https://github.com/zesterer/chumsky/issues/745
- https://github.com/zesterer/chumsky/releases/tag/0.10

## Tests

- The goal is for all existing tests to pass when running the `chumsky-10` feature (and only using `chumsky-10` for the lexer)
- Do not disable tests that are failing due to the new lexer.

- After each group of changes, run:
   ```
   # cargo check for this package
   cargo check -p prqlc-parser --features chumsky-10

   # tests for this module
   cargo insta test --check -p prqlc-parser --features chumsky-10 -- lexer::

   # confirm the existing tests still pass without the `chumsky-10` feature
   cargo insta test --check -p prqlc-parser
   ```

- and the linting instructions in `CLAUDE.md`

*/

use chumsky_0_10::error::Simple;
use chumsky_0_10::extra;
use chumsky_0_10::input::Stream;
use chumsky_0_10::prelude::*;
use chumsky_0_10::primitive::{choice, end, just, none_of, one_of};
use chumsky_0_10::Parser;

use super::lr::{Literal, Token, TokenKind, Tokens, ValueAndUnit};
use crate::error::{Error, ErrorSource, Reason, WithErrorInfo};
use crate::span::Span;

type E = Error;
type ParserInput<'a> = Stream<std::str::Chars<'a>>;
// Use the extra::Default type for error handling
type ParserError = extra::Default;

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, _source_id: u16) -> (Option<Vec<Token>>, Vec<E>) {
    // Create a stream for the characters
    let stream = Stream::from_iter(source.chars());

    // In chumsky 0.10, we parse directly from the stream using extra::Default
    let result = lexer().parse(stream);

    if let Some(tokens) = result.output() {
        (Some(insert_start(tokens.to_vec())), vec![])
    } else {
        // Create a generic error for the lexing failure
        let errors = vec![Error::new(Reason::Unexpected {
            found: "Lexer error".to_string(),
        })
        .with_source(ErrorSource::Lexer("Failed to parse".to_string()))];

        (None, errors)
    }
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<Tokens, Vec<E>> {
    // Create a stream for the characters
    let stream = Stream::from_iter(source.chars());

    // In chumsky 0.10, we parse directly from the stream
    let result = lexer().parse(stream);

    if let Some(tokens) = result.output() {
        Ok(Tokens(insert_start(tokens.to_vec())))
    } else {
        // Create a generic error for the lexing failure
        let errors = vec![Error::new(Reason::Unexpected {
            found: if !source.is_empty() {
                source.chars().next().unwrap().to_string()
            } else {
                "Empty input".to_string()
            },
        })
        .with_source(ErrorSource::Lexer("Failed to parse".to_string()))];

        Err(errors)
    }
}

/// Insert a start token so later stages can treat the start of a file like a newline
fn insert_start(tokens: Vec<Token>) -> Vec<Token> {
    std::iter::once(Token {
        kind: TokenKind::Start,
        span: 0..0,
    })
    .chain(tokens)
    .collect()
}

// Convert chumsky 0.10 error to our Error type
fn convert_lexer_error(
    source: &str,
    e: Simple<chumsky_0_10::span::SimpleSpan<usize>>,
    source_id: u16,
) -> Error {
    // In Chumsky 0.10, errors have a different structure
    let span_start = e.span().start;
    let span_end = e.span().end;

    // Try to extract the problematic character
    let found = if span_start < source.len() {
        // Get the character at the span position if possible
        source.chars().nth(span_start).map_or_else(
            || format!("Error at position {}", span_start),
            |c| format!("{}", c),
        )
    } else {
        // If span is out of bounds, provide a generic error message
        format!("Error at end of input")
    };

    let span = Some(Span {
        start: span_start,
        end: span_end,
        source_id,
    });

    Error::new(Reason::Unexpected { found })
        .with_span(span)
        .with_source(ErrorSource::Lexer(format!("{:?}", e)))
}

/// Lex chars to tokens until the end of the input
pub fn lexer<'src>() -> impl Parser<'src, ParserInput<'src>, Vec<Token>, ParserError> {
    lex_token()
        .repeated()
        .collect()
        .then_ignore(ignored())
        .then_ignore(end())
}

// Parsers for date and time components
fn digits<'src>(count: usize) -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    any().filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
        .collect::<Vec<char>>()
}

fn date_inner<'src>() -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    digits(4)
        .then(just('-'))
        .then(digits(2))
        .then(just('-'))
        .then(digits(2))
        .map(|((((year, dash1), month), dash2), day)| {
            // Flatten the tuple structure
            let mut result = Vec::new();
            result.extend(year.iter().cloned());
            result.push(dash1);
            result.extend(month.iter().cloned());
            result.push(dash2);
            result.extend(day.iter().cloned());
            result
        })
        .boxed()
}

fn time_inner<'src>() -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    digits(2)
        // minutes
        .then(
            just(':')
                .then(digits(2))
                .map(|(colon, min)| {
                    let mut result = Vec::new();
                    result.push(colon);
                    result.extend(min.iter().cloned());
                    result
                })
                .or_not()
                .map(|opt| opt.unwrap_or_default()),
        )
        // seconds
        .then(
            just(':')
                .then(digits(2))
                .map(|(colon, sec)| {
                    let mut result = Vec::new();
                    result.push(colon);
                    result.extend(sec.iter().cloned());
                    result
                })
                .or_not()
                .map(|opt| opt.unwrap_or_default()),
        )
        // milliseconds
        .then(
            just('.')
                .then(
                    any().filter(|c: &char| c.is_ascii_digit())
                        .repeated()
                        .at_least(1)
                        .at_most(6)
                        .collect::<Vec<char>>(),
                )
                .map(|(dot, digits)| {
                    let mut result = Vec::new();
                    result.push(dot);
                    result.extend(digits.iter().cloned());
                    result
                })
                .or_not()
                .map(|opt| opt.unwrap_or_default()),
        )
        // timezone offset
        .then(
            choice((
                // Either just `Z`
                just('Z').map(|x| vec![x]),
                // Or an offset, such as `-05:00` or `-0500`
                one_of("-+")
                    .then(
                        digits(2)
                            .then(just(':').or_not().then(digits(2)).map(|(opt_colon, min)| {
                                let mut result = Vec::new();
                                if let Some(colon) = opt_colon {
                                    result.push(colon);
                                }
                                result.extend(min.iter().cloned());
                                result
                            }))
                            .map(|(hrs, mins)| {
                                let mut result = Vec::new();
                                result.extend(hrs.iter().cloned());
                                result.extend(mins.iter().cloned());
                                result
                            }),
                    )
                    .map(|(sign, offset)| {
                        let mut result = vec![sign];
                        result.extend(offset.iter().cloned());
                        result
                    }),
            ))
            .or_not()
            .map(|opt| opt.unwrap_or_default()),
        )
        .map(|((((hours, minutes), seconds), milliseconds), timezone)| {
            let mut result = Vec::new();
            result.extend(hours.iter().cloned());
            result.extend(minutes.iter().cloned());
            result.extend(seconds.iter().cloned());
            result.extend(milliseconds.iter().cloned());
            result.extend(timezone.iter().cloned());
            result
        })
        .boxed()
}

/// Lex chars to a single token
fn lex_token<'src>() -> impl Parser<'src, ParserInput<'src>, Token, ParserError> {
    let control_multi = choice((
        just("->").map(|_| TokenKind::ArrowThin),
        just("=>").map(|_| TokenKind::ArrowFat),
        just("==").map(|_| TokenKind::Eq),
        just("!=").map(|_| TokenKind::Ne),
        just(">=").map(|_| TokenKind::Gte),
        just("<=").map(|_| TokenKind::Lte),
        just("~=").map(|_| TokenKind::RegexSearch),
        just("&&").then_ignore(end_expr()).map(|_| TokenKind::And),
        just("||").then_ignore(end_expr()).map(|_| TokenKind::Or),
        just("??").map(|_| TokenKind::Coalesce),
        just("//").map(|_| TokenKind::DivInt),
        just("**").map(|_| TokenKind::Pow),
        // Handle @ annotations properly - match both @{...} and standalone @
        just("@")
            .then(just("{").not().rewind())
            .map(|_| TokenKind::Annotate),
    ));

    let control = one_of("></%=+-*[]().,:|!{}").map(TokenKind::Control);

    let ident = ident_part().map(TokenKind::Ident);

    let keyword = choice((
        just("let"),
        just("into"),
        just("case"),
        just("prql"),
        just("type"),
        just("module"),
        just("internal"),
        just("func"),
        just("import"),
        just("enum"),
    ))
    .then_ignore(end_expr())
    .map(|x| x.to_string())
    .map(TokenKind::Keyword);

    let literal = literal().map(TokenKind::Literal);

    // Date/time literals starting with @
    let date_token = just('@')
        // Not an annotation (@{)
        .then(just('{').not().rewind())
        .ignore_then(choice((
            // datetime: @2022-01-01T12:00
            date_inner()
                .then(just('T'))
                .then(time_inner())
                .then_ignore(end_expr())
                .map(|((date, t), time)| {
                    let mut result = Vec::new();
                    result.extend(date.iter().cloned());
                    result.push(t);
                    result.extend(time.iter().cloned());
                    Literal::Timestamp(String::from_iter(result))
                }),
            // date: @2022-01-01
            date_inner()
                .then_ignore(end_expr())
                .map(|chars| Literal::Date(chars.into_iter().collect::<String>())),
            // time: @12:00
            time_inner()
                .then_ignore(end_expr())
                .map(|chars| Literal::Time(chars.into_iter().collect::<String>())),
        )))
        .map(TokenKind::Literal);

    let param = just('$')
        .ignore_then(
            any().filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.')
                .repeated()
                .collect::<String>(),
        )
        .map(TokenKind::Param);

    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| TokenKind::Interpolation(c, s));

    let token = choice((
        line_wrap(),
        newline().map(|_| TokenKind::NewLine),
        control_multi,
        interpolation,
        param,
        date_token, // Add date token before control/literal to ensure @ is handled properly
        control,
        literal,
        keyword,
        ident,
        comment(),
    ));

    // Simple approach for ranges - just use the span as is
    let range = just("..").map_with(|_, extra| {
        let span: chumsky_0_10::span::SimpleSpan = extra.span();
        Token {
            kind: TokenKind::Range {
                bind_left: true,
                bind_right: true,
            },
            span: span.start()..span.end(),
        }
    });

    // For other tokens, use map_with to capture span information
    let other_tokens = token.map_with(|kind, extra| {
        let span: chumsky_0_10::span::SimpleSpan = extra.span();
        Token {
            kind,
            span: span.start()..span.end(),
        }
    });

    // Choose between range and regular tokens
    // We need to match the whitespace pattern from chumsky_0_9.rs
    choice((
        // Handle range with proper whitespace
        ignored().ignore_then(range),
        // Handle other tokens with proper whitespace
        ignored().ignore_then(other_tokens),
    ))
}

fn ignored<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    whitespace().repeated().ignored()
}

fn whitespace<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    any().filter(|x: &char| *x == ' ' || *x == '\t')
        .repeated()
        .at_least(1)
        .ignored()
}

// Custom newline parser for Stream<char> since it doesn't implement StrInput
fn newline<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    just('\n')
        .or(just('\r').then_ignore(just('\n').or_not()))
        .ignored()
}

fn line_wrap<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    newline()
        .ignore_then(
            whitespace()
                .repeated()
                .ignore_then(comment())
                .then_ignore(newline())
                .repeated()
                .collect(),
        )
        .then_ignore(whitespace().repeated())
        .then_ignore(just('\\'))
        .map(TokenKind::LineWrap)
}

fn comment<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    just('#').ignore_then(choice((
        // One option would be to check that doc comments have new lines in the
        // lexer (we currently do in the parser); which would give better error
        // messages?
        just('!').ignore_then(
            // Replacement for take_until - capture chars until we see a newline
            any().filter(|c: &char| *c != '\n' && *c != '\r')
                .repeated()
                .collect::<String>()
                .map(TokenKind::DocComment),
        ),
        // Replacement for take_until - capture chars until we see a newline
        any().filter(|c: &char| *c != '\n' && *c != '\r')
            .repeated()
            .collect::<String>()
            .map(TokenKind::Comment),
    )))
}

pub fn ident_part<'src>() -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    // Create a parser for a single alphanumeric/underscore character after the first
    let rest_char = any().filter(|c: &char| c.is_alphanumeric() || *c == '_');

    // Parse a word: an alphabetic/underscore followed by alphanumerics/underscores
    let plain = any().filter(|c: &char| c.is_alphabetic() || *c == '_')
        .then(rest_char.repeated().collect::<Vec<char>>())
        .map(|(first, rest)| {
            let mut chars = vec![first];
            chars.extend(rest);
            chars.into_iter().collect::<String>()
        });

    // Parse a backtick-quoted identifier
    let backtick = none_of('`')
        .repeated()
        .collect::<Vec<char>>()
        .delimited_by(just('`'), just('`'))
        .map(|chars| chars.into_iter().collect::<String>());

    choice((plain, backtick))
}

pub fn literal<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    let binary_notation = just("0b")
        .then_ignore(just("_").or_not())
        .ignore_then(
            any().filter(|c: &char| *c == '0' || *c == '1')
                .repeated()
                .at_least(1)
                .at_most(32)
                .collect::<String>()
                .map(|digits: String| match i64::from_str_radix(&digits, 2) {
                    Ok(i) => Literal::Integer(i),
                    Err(_) => Literal::Integer(0), // Default to 0 on error for now
                }),
        )
        .labelled("number");

    let hexadecimal_notation = just("0x")
        .then_ignore(just("_").or_not())
        .ignore_then(
            any().filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .at_least(1)
                .at_most(12)
                .collect::<String>()
                .map(|digits: String| match i64::from_str_radix(&digits, 16) {
                    Ok(i) => Literal::Integer(i),
                    Err(_) => Literal::Integer(0), // Default to 0 on error for now
                }),
        )
        .labelled("number");

    let octal_notation = just("0o")
        .then_ignore(just("_").or_not())
        .ignore_then(
            any().filter(|c: &char| ('0'..='7').contains(c))
                .repeated()
                .at_least(1)
                .at_most(12)
                .collect::<String>()
                .map(|digits: String| match i64::from_str_radix(&digits, 8) {
                    Ok(i) => Literal::Integer(i),
                    Err(_) => Literal::Integer(0), // Default to 0 on error for now
                }),
        )
        .labelled("number");

    let exp = one_of("eE")
        .then(
            one_of("+-")
                .or_not()
                .then(
                    any().filter(|c: &char| c.is_ascii_digit())
                        .repeated()
                        .at_least(1)
                        .collect::<Vec<char>>(),
                )
                .map(|(sign_opt, digits)| {
                    let mut result = Vec::new();
                    if let Some(sign) = sign_opt {
                        result.push(sign);
                    }
                    result.extend(digits.iter().cloned());
                    result
                }),
        )
        .map(|(e, rest)| {
            let mut result = vec![e];
            result.extend(rest);
            result
        });

    // Define integer parsing separately so it can be reused
    let parse_integer = || {
        any().filter(|c: &char| c.is_ascii_digit() && *c != '0')
            .then(
                any().filter(|c: &char| c.is_ascii_digit() || *c == '_')
                    .repeated()
                    .collect::<Vec<char>>(),
            )
            .map(|(first, rest)| {
                let mut chars = vec![first];
                chars.extend(rest);
                chars
            })
            .or(just('0').map(|c| vec![c]))
    };

    let integer = parse_integer();

    let frac = just('.')
        .then(any().filter(|c: &char| c.is_ascii_digit()))
        .then(
            any().filter(|c: &char| c.is_ascii_digit() || *c == '_')
                .repeated()
                .collect::<Vec<char>>(),
        )
        .map(|((dot, first), rest)| {
            let mut result = vec![dot, first];
            result.extend(rest);
            result
        });

    let number = integer
        .then(frac.or_not().map(|opt| opt.unwrap_or_default()))
        .then(exp.or_not().map(|opt| opt.unwrap_or_default()))
        .map(|((mut int_part, mut frac_part), mut exp_part)| {
            let mut result = Vec::new();
            result.append(&mut int_part);
            result.append(&mut frac_part);
            result.append(&mut exp_part);
            result
        })
        .map(|chars: Vec<char>| {
            let str = chars.into_iter().filter(|c| *c != '_').collect::<String>();

            if let Ok(i) = str.parse::<i64>() {
                Literal::Integer(i)
            } else if let Ok(f) = str.parse::<f64>() {
                Literal::Float(f)
            } else {
                Literal::Integer(0) // Default to 0 on error for now
            }
        })
        .labelled("number");

    let string = quoted_string(true).map(Literal::String);

    // Raw string needs to be more explicit to avoid being interpreted as a function call
    let raw_string = just("r")
        .then(choice((just('\''), just('"'))))
        .then(
            any().filter(move |c: &char| *c != '\'' && *c != '"' && *c != '\n' && *c != '\r')
                .repeated()
                .collect::<Vec<char>>(),
        )
        .then(choice((just('\''), just('"'))))
        .map(|(((_, _), chars), _)| chars.into_iter().collect::<String>())
        .map(Literal::RawString);

    let bool = (just("true").map(|_| true))
        .or(just("false").map(|_| false))
        .then_ignore(end_expr())
        .map(Literal::Boolean);

    let null = just("null").map(|_| Literal::Null).then_ignore(end_expr());

    let value_and_unit = parse_integer()
        .then(choice((
            just("microseconds"),
            just("milliseconds"),
            just("seconds"),
            just("minutes"),
            just("hours"),
            just("days"),
            just("weeks"),
            just("months"),
            just("years"),
        )))
        .then_ignore(end_expr())
        .map(|(number, unit): (Vec<char>, &str)| {
            let str = number.into_iter().filter(|c| *c != '_').collect::<String>();
            if let Ok(n) = str.parse::<i64>() {
                let unit = unit.to_string();
                ValueAndUnit { n, unit }
            } else {
                // Default to 1 with the unit on error
                ValueAndUnit {
                    n: 1,
                    unit: unit.to_string(),
                }
            }
        })
        .map(Literal::ValueAndUnit);

    // Date/time literals are now handled directly in the lexer token parser

    choice((
        binary_notation,
        hexadecimal_notation,
        octal_notation,
        string,
        raw_string,
        value_and_unit,
        number,
        bool,
        null,
    ))
}

pub fn quoted_string<'src>(
    escaped: bool,
) -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    choice((
        // Handle triple-quoted strings (multi-line) - this is why tests were failing
        quoted_triple_string(escaped),
        quoted_string_of_quote(&'"', escaped, false),
        quoted_string_of_quote(&'\'', escaped, false),
    ))
    .map(|chars| chars.into_iter().collect::<String>())
    .labelled("string")
}

// Handle triple quoted strings with proper escaping
fn quoted_triple_string<'src>(
    escaped: bool,
) -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    // Parser for triple single quotes
    let triple_single = just('\'')
        .then(just('\''))
        .then(just('\''))
        .ignore_then(
            choice((
                // Handle escaped characters if escaping is enabled
                just('\\')
                    .then(choice((
                        just('\'').map(|_| '\''),
                        just('\\').map(|_| '\\'),
                        just('n').map(|_| '\n'),
                        just('r').map(|_| '\r'),
                        just('t').map(|_| '\t'),
                    )))
                    .map(|(_, c)| c),
                // Normal characters except triple quotes
                any().filter(move |c: &char| *c != '\'' || !escaped),
            ))
            .repeated()
            .collect::<Vec<char>>(),
        )
        .then_ignore(just('\'').then(just('\'')).then(just('\'')));

    // Parser for triple double quotes
    let triple_double = just('"')
        .then(just('"'))
        .then(just('"'))
        .ignore_then(
            choice((
                // Handle escaped characters if escaping is enabled
                just('\\')
                    .then(choice((
                        just('"').map(|_| '"'),
                        just('\\').map(|_| '\\'),
                        just('n').map(|_| '\n'),
                        just('r').map(|_| '\r'),
                        just('t').map(|_| '\t'),
                    )))
                    .map(|(_, c)| c),
                // Normal characters except triple quotes
                any().filter(move |c: &char| *c != '"' || !escaped),
            ))
            .repeated()
            .collect::<Vec<char>>(),
        )
        .then_ignore(just('"').then(just('"')).then(just('"')));

    // Choose between triple single quotes or triple double quotes
    choice((triple_single, triple_double))
}

fn quoted_string_of_quote<'src, 'a>(
    quote: &'a char,
    escaping: bool,
    allow_multiline: bool,
) -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> + 'a
where
    'src: 'a,
{
    let q = *quote;

    // Parser for non-quote characters
    let regular_char = if allow_multiline {
        any().filter(move |c: &char| *c != q && *c != '\\').boxed()
    } else {
        any().filter(move |c: &char| *c != q && *c != '\n' && *c != '\r' && *c != '\\').boxed()
    };

    // Parser for escaped characters if escaping is enabled
    let escaped_char = choice((
        just('\\').ignore_then(just(q)),                 // Escaped quote
        just('\\').ignore_then(just('\\')),              // Escaped backslash
        just('\\').ignore_then(just('n')).map(|_| '\n'), // Newline
        just('\\').ignore_then(just('r')).map(|_| '\r'), // Carriage return
        just('\\').ignore_then(just('t')).map(|_| '\t'), // Tab
        escaped_character(),                             // Handle all other escape sequences
    ));

    // Choose the right character parser based on whether escaping is enabled
    let char_parser = if escaping {
        choice((escaped_char, regular_char)).boxed()
    } else {
        regular_char.boxed()
    };

    // Complete string parser
    just(q)
        .ignore_then(char_parser.repeated().collect())
        .then_ignore(just(q))
}

// This function will be used for more advanced string parsing
// when we implement the full set of string features from 0.9
#[allow(dead_code)]
fn escaped_character<'src>() -> impl Parser<'src, ParserInput<'src>, char, ParserError> {
    just('\\').ignore_then(choice((
        just('\\'),
        just('/'),
        just('b').map(|_| '\x08'),
        just('f').map(|_| '\x0C'),
        just('n').map(|_| '\n'),
        just('r').map(|_| '\r'),
        just('t').map(|_| '\t'),
        (just("u{").ignore_then(
            any().filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .at_least(1)
                .at_most(6)
                .collect::<String>()
                .map(|digits: String| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap_or(0)).unwrap_or('?')
                    // Default to ? on error
                })
                .then_ignore(just('}')),
        )),
        (just('x').ignore_then(
            any().filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(2)
                .collect::<String>()
                .map(|digits: String| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap_or(0)).unwrap_or('?')
                    // Default to ? on error
                }),
        )),
    )))
}

fn end_expr<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    choice((
        end(),
        one_of(",)]}\t >").map(|_| ()),
        newline(),
        just("..").map(|_| ()),
    ))
    .rewind()
}
