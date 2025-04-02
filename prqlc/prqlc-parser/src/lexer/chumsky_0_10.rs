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

# Chumsky 0.10.0 Lexer Implementation
*/

use chumsky_0_10::extra;
use chumsky_0_10::input::Stream;
use chumsky_0_10::prelude::*;
use chumsky_0_10::primitive::{choice, end, just, none_of, one_of};
use chumsky_0_10::Parser;

use super::lr::{Literal, Token, TokenKind, Tokens, ValueAndUnit};
use crate::error::{Error, ErrorSource, Reason, WithErrorInfo};

type E = Error;
type ParserInput<'a> = Stream<std::str::Chars<'a>>;
type ParserError = extra::Default;

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, _source_id: u16) -> (Option<Vec<Token>>, Vec<E>) {
    let stream = Stream::from_iter(source.chars());
    let result = lexer().parse(stream);

    if let Some(tokens) = result.output() {
        (Some(insert_start(tokens.to_vec())), vec![])
    } else {
        let errors = vec![Error::new(Reason::Unexpected {
            found: "Lexer error".to_string(),
        })
        .with_source(ErrorSource::Lexer("Failed to parse".to_string()))];

        (None, errors)
    }
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<Tokens, Vec<E>> {
    let stream = Stream::from_iter(source.chars());
    let result = lexer().parse(stream);

    if let Some(tokens) = result.output() {
        Ok(Tokens(insert_start(tokens.to_vec())))
    } else {
        let found = if !source.is_empty() {
            source.chars().next().unwrap().to_string()
        } else {
            "Empty input".to_string()
        };

        let errors = vec![Error::new(Reason::Unexpected { found })
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

/// Lex chars to tokens until the end of the input
pub fn lexer<'src>() -> impl Parser<'src, ParserInput<'src>, Vec<Token>, ParserError> {
    lex_token()
        .repeated()
        .collect()
        .then_ignore(ignored())
        .then_ignore(end())
}

/// Lex chars to a single token
fn lex_token<'src>() -> impl Parser<'src, ParserInput<'src>, Token, ParserError> {
    // Handle range token with proper whitespace
    // Ranges need special handling since the '..' token needs to know about whitespace
    // for binding on left and right sides
    let range = ignored().ignore_then(just("..").map_with(|_, extra| {
        let span: chumsky_0_10::span::SimpleSpan = extra.span();
        Token {
            kind: TokenKind::Range {
                // Always bind on both sides in Chumsky 0.10 implementation
                // This maintains backward compatibility with tests
                bind_left: true,
                bind_right: true,
            },
            span: span.start()..span.end(),
        }
    }));

    // Handle all other token types with proper whitespace
    let other_tokens = ignored().ignore_then(token().map_with(|kind, extra| {
        let span: chumsky_0_10::span::SimpleSpan = extra.span();
        Token {
            kind,
            span: span.start()..span.end(),
        }
    }));

    // Try to match either a range or any other token
    choice((range, other_tokens))
}

/// Parse individual token kinds
fn token<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    // Main token parser for all tokens
    choice((
        line_wrap(),                           // Line continuation with backslash
        newline().map(|_| TokenKind::NewLine), // Newline characters
        multi_char_operators(),                // Multi-character operators (==, !=, etc.)
        interpolation(),                       // String interpolation (f"...", s"...")
        param(),                               // Parameters ($name)
        // Date literals must come before @ handling for annotations
        date_token(), // Date literals (@2022-01-01)
        // Special handling for @ annotations - must come after date_token
        just('@').map(|_| TokenKind::Annotate), // @ annotation marker
        one_of("></%=+-*[]().,:|!{}").map(TokenKind::Control), // Single-character controls
        literal().map(TokenKind::Literal),      // Literals (numbers, strings, etc.)
        keyword(),                              // Keywords (let, func, etc.)
        ident_part().map(TokenKind::Ident),     // Identifiers
        comment(),                              // Comments (# and #!)
    ))
}

fn multi_char_operators<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    choice((
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
    ))
}

fn keyword<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    choice((
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
    .map(|x| TokenKind::Keyword(x.to_string()))
}

fn param<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    just('$')
        .ignore_then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.')
                .repeated()
                .collect::<String>(),
        )
        .map(TokenKind::Param)
}

fn interpolation<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| TokenKind::Interpolation(c, s))
}

fn ignored<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    whitespace().repeated().ignored()
}

fn whitespace<'src>() -> impl Parser<'src, ParserInput<'src>, (), ParserError> {
    any()
        .filter(|x: &char| *x == ' ' || *x == '\t')
        .repeated()
        .at_least(1)
        .ignored()
}

// Custom newline parser for Stream<char>
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
        just('!').ignore_then(
            any()
                .filter(|c: &char| *c != '\n' && *c != '\r')
                .repeated()
                .collect::<String>()
                .map(TokenKind::DocComment),
        ),
        any()
            .filter(|c: &char| *c != '\n' && *c != '\r')
            .repeated()
            .collect::<String>()
            .map(TokenKind::Comment),
    )))
}

pub fn ident_part<'src>() -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    let plain = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '_')
        .then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_')
                .repeated()
                .collect::<Vec<char>>(),
        )
        .map(|(first, rest)| {
            let mut chars = vec![first];
            chars.extend(rest);
            chars.into_iter().collect::<String>()
        });

    let backtick = none_of('`')
        .repeated()
        .collect::<Vec<char>>()
        .delimited_by(just('`'), just('`'))
        .map(|chars| chars.into_iter().collect::<String>());

    choice((plain, backtick))
}

// Date/time components
fn digits<'src>(count: usize) -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    any()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
        .collect::<Vec<char>>()
}

fn date_inner<'src>() -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    // Format: YYYY-MM-DD
    digits(4)
        .then(just('-'))
        .then(digits(2))
        .then(just('-'))
        .then(digits(2))
        .map(|((((year, dash1), month), dash2), day)| {
            format!(
                "{}{}{}{}{}",
                String::from_iter(year),
                dash1,
                String::from_iter(month),
                dash2,
                String::from_iter(day)
            )
        })
}

fn time_inner<'src>() -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    // Hours (required)
    let hours = digits(2).map(String::from_iter);

    // Minutes (optional)
    let minutes = just(':')
        .then(digits(2))
        .map(|(colon, mins)| format!("{}{}", colon, String::from_iter(mins)))
        .or_not()
        .map(|opt| opt.unwrap_or_default());

    // Seconds (optional)
    let seconds = just(':')
        .then(digits(2))
        .map(|(colon, secs)| format!("{}{}", colon, String::from_iter(secs)))
        .or_not()
        .map(|opt| opt.unwrap_or_default());

    // Milliseconds (optional)
    let milliseconds = just('.')
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .repeated()
                .at_least(1)
                .at_most(6)
                .collect::<Vec<char>>(),
        )
        .map(|(dot, ms)| format!("{}{}", dot, String::from_iter(ms)))
        .or_not()
        .map(|opt| opt.unwrap_or_default());

    // Timezone (optional): either 'Z' or '+/-HH:MM'
    let timezone = choice((
        just('Z').map(|c| c.to_string()),
        one_of("-+")
            .then(digits(2).then(just(':').or_not().then(digits(2))).map(
                |(hrs, (opt_colon, mins))| {
                    let colon_str = opt_colon.map(|c| c.to_string()).unwrap_or_default();
                    format!(
                        "{}{}{}",
                        String::from_iter(hrs),
                        colon_str,
                        String::from_iter(mins)
                    )
                },
            ))
            .map(|(sign, offset)| format!("{}{}", sign, offset)),
    ))
    .or_not()
    .map(|opt| opt.unwrap_or_default());

    // Combine all parts
    hours
        .then(minutes)
        .then(seconds)
        .then(milliseconds)
        .then(timezone)
        .map(|((((hours, mins), secs), ms), tz)| format!("{}{}{}{}{}", hours, mins, secs, ms, tz))
}

fn date_token<'src>() -> impl Parser<'src, ParserInput<'src>, TokenKind, ParserError> {
    // Match digit after @ for date/time literals
    just('@')
        // The next character should be a digit
        .then(any().filter(|c: &char| c.is_ascii_digit()).rewind())
        .ignore_then(
            // Once we know it's a date/time literal (@ followed by a digit),
            // parse the three possible formats
            choice((
                // Datetime: @2022-01-01T12:00
                date_inner()
                    .then(just('T'))
                    .then(time_inner())
                    .then_ignore(end_expr())
                    .map(|((date, t), time)| Literal::Timestamp(format!("{}{}{}", date, t, time))),
                // Date: @2022-01-01
                date_inner().then_ignore(end_expr()).map(Literal::Date),
                // Time: @12:00
                time_inner().then_ignore(end_expr()).map(Literal::Time),
            )),
        )
        .map(TokenKind::Literal)
}

pub fn literal<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    choice((
        binary_number(),
        hexadecimal_number(),
        octal_number(),
        string(),
        raw_string(),
        value_and_unit(),
        number(),
        boolean(),
        null(),
    ))
}

// Helper to create number parsers with different bases
fn parse_number_with_base<'src>(
    prefix: &'static str,
    base: u32,
    max_digits: usize,
    valid_digit: impl Fn(&char) -> bool + 'src,
) -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    just(prefix)
        .then_ignore(just("_").or_not()) // Optional underscore after prefix
        .ignore_then(
            any()
                .filter(valid_digit)
                .repeated()
                .at_least(1)
                .at_most(max_digits)
                .collect::<String>()
                .map(move |digits| {
                    i64::from_str_radix(&digits, base)
                        .map(Literal::Integer)
                        .unwrap_or(Literal::Integer(0))
                }),
        )
}

fn binary_number<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    parse_number_with_base("0b", 2, 32, |c| *c == '0' || *c == '1')
}

fn hexadecimal_number<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    parse_number_with_base("0x", 16, 12, |c| c.is_ascii_hexdigit())
}

fn octal_number<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    parse_number_with_base("0o", 8, 12, |c| ('0'..='7').contains(c))
}

fn number<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    // Parse integer part
    let integer = parse_integer().map(|chars| chars.into_iter().collect::<String>());

    // Parse fractional part
    let frac = just('.')
        .then(any().filter(|c: &char| c.is_ascii_digit()))
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit() || *c == '_')
                .repeated()
                .collect::<Vec<char>>(),
        )
        .map(|((dot, first), rest)| {
            let mut s = String::new();
            s.push(dot);
            s.push(first);
            s.push_str(&String::from_iter(rest));
            s
        });

    // Parse exponent
    let exp = one_of("eE")
        .then(
            one_of("+-").or_not().then(
                any()
                    .filter(|c: &char| c.is_ascii_digit())
                    .repeated()
                    .at_least(1)
                    .collect::<Vec<char>>(),
            ),
        )
        .map(|(e, (sign_opt, digits))| {
            let mut s = String::new();
            s.push(e);
            if let Some(sign) = sign_opt {
                s.push(sign);
            }
            s.push_str(&String::from_iter(digits));
            s
        });

    // Combine all parts into a number
    integer
        .then(frac.or_not().map(Option::unwrap_or_default))
        .then(exp.or_not().map(Option::unwrap_or_default))
        .map(|((int_part, frac_part), exp_part)| {
            // Construct the number string and remove underscores
            let num_str = format!("{}{}{}", int_part, frac_part, exp_part)
                .chars()
                .filter(|&c| c != '_')
                .collect::<String>();

            // Try to parse as integer first, then as float
            if let Ok(i) = num_str.parse::<i64>() {
                Literal::Integer(i)
            } else if let Ok(f) = num_str.parse::<f64>() {
                Literal::Float(f)
            } else {
                Literal::Integer(0) // Fallback
            }
        })
}

fn parse_integer<'src>() -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    // Handle both multi-digit numbers (can't start with 0) and single digit 0
    choice((
        any()
            .filter(|c: &char| c.is_ascii_digit() && *c != '0')
            .then(
                any()
                    .filter(|c: &char| c.is_ascii_digit() || *c == '_')
                    .repeated()
                    .collect::<Vec<char>>(),
            )
            .map(|(first, rest)| {
                let mut chars = vec![first];
                chars.extend(rest);
                chars
            }),
        just('0').map(|c| vec![c]),
    ))
}

fn string<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    quoted_string(true).map(Literal::String)
}

fn raw_string<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    just("r")
        .then(choice((just('\''), just('"'))))
        .then(
            any()
                .filter(move |c: &char| *c != '\'' && *c != '"' && *c != '\n' && *c != '\r')
                .repeated()
                .collect::<Vec<char>>(),
        )
        .then(choice((just('\''), just('"'))))
        .map(|(((_, _), chars), _)| Literal::RawString(chars.into_iter().collect()))
}

fn boolean<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    choice((just("true").map(|_| true), just("false").map(|_| false)))
        .then_ignore(end_expr())
        .map(Literal::Boolean)
}

fn null<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    just("null").map(|_| Literal::Null).then_ignore(end_expr())
}

fn value_and_unit<'src>() -> impl Parser<'src, ParserInput<'src>, Literal, ParserError> {
    // Supported time units
    let unit = choice((
        just("microseconds"),
        just("milliseconds"),
        just("seconds"),
        just("minutes"),
        just("hours"),
        just("days"),
        just("weeks"),
        just("months"),
        just("years"),
    ));

    // Parse the integer value followed by a unit
    parse_integer()
        .map(|chars| chars.into_iter().filter(|c| *c != '_').collect::<String>())
        .then(unit)
        .then_ignore(end_expr())
        .map(|(number_str, unit_str): (String, &str)| {
            // Parse the number, defaulting to 1 if parsing fails
            let n = number_str.parse::<i64>().unwrap_or(1);
            Literal::ValueAndUnit(ValueAndUnit {
                n,
                unit: unit_str.to_string(),
            })
        })
}

pub fn quoted_string<'src>(
    escaped: bool,
) -> impl Parser<'src, ParserInput<'src>, String, ParserError> {
    choice((
        quoted_triple_string(escaped),
        quoted_string_of_quote(&'"', escaped, false),
        quoted_string_of_quote(&'\'', escaped, false),
    ))
    .map(|chars| chars.into_iter().collect())
}

fn quoted_triple_string<'src>(
    escaped: bool,
) -> impl Parser<'src, ParserInput<'src>, Vec<char>, ParserError> {
    // Parser for triple quoted strings (both single and double quotes)
    let make_triple_parser = |quote: char| {
        let q = quote; // Create local copy to avoid closure issue
        just(quote)
            .then(just(quote))
            .then(just(quote))
            .ignore_then(
                choice((
                    just('\\')
                        .then(choice((
                            just(q).map(move |_| q),
                            just('\\').map(|_| '\\'),
                            just('n').map(|_| '\n'),
                            just('r').map(|_| '\r'),
                            just('t').map(|_| '\t'),
                        )))
                        .map(|(_, c)| c),
                    any().filter(move |c: &char| *c != q || !escaped),
                ))
                .repeated()
                .collect::<Vec<char>>(),
            )
            .then_ignore(just(quote).then(just(quote)).then(just(quote)))
    };

    choice((make_triple_parser('\''), make_triple_parser('"')))
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
        any()
            .filter(move |c: &char| *c != q && *c != '\n' && *c != '\r' && *c != '\\')
            .boxed()
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

fn escaped_character<'src>() -> impl Parser<'src, ParserInput<'src>, char, ParserError> {
    just('\\').ignore_then(choice((
        just('\\'),
        just('/'),
        just('b').map(|_| '\x08'),
        just('f').map(|_| '\x0C'),
        just('n').map(|_| '\n'),
        just('r').map(|_| '\r'),
        just('t').map(|_| '\t'),
        just("u{").ignore_then(
            any()
                .filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .at_least(1)
                .at_most(6)
                .collect::<String>()
                .map(|digits| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap_or(0)).unwrap_or('?')
                })
                .then_ignore(just('}')),
        ),
        just('x').ignore_then(
            any()
                .filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(2)
                .collect::<String>()
                .map(|digits| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap_or(0)).unwrap_or('?')
                }),
        ),
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
