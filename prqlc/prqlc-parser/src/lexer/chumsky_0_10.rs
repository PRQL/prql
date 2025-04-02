/*
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
// Define a custom error type with the `Simple` error type from chumsky_0_10
type ParserError<'a> = extra::Err<Simple<'a, char>>;

/// Convert a chumsky Simple error to our internal Error type
fn convert_lexer_error(error: &Simple<'_, char>, source_id: u16) -> E {
    // Get span information from the Simple error
    let span = error.span();
    let error_start = span.start();
    let error_end = span.end();

    // Get the found token from the Simple error
    let found = error
        .found()
        .map_or_else(|| "end of input".to_string(), |c| format!("'{}'", c));

    // Create a new Error with the extracted information
    Error::new(Reason::Unexpected {
        found: found.clone(),
    })
    .with_span(Some(crate::span::Span {
        start: error_start,
        end: error_end,
        source_id,
    }))
    .with_source(ErrorSource::Lexer(format!(
        "Unexpected {} at position {}..{}",
        found, error_start, error_end
    )))
}

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, source_id: u16) -> (Option<Vec<Token>>, Vec<E>) {
    let stream = Stream::from_iter(source.chars());
    let result = lexer().parse(stream).into_result();

    match result {
        Ok(tokens) => (Some(insert_start(tokens.to_vec())), vec![]),
        Err(errors) => {
            // Convert chumsky Simple errors to our Error type
            let errors = errors
                .into_iter()
                .map(|error| convert_lexer_error(&error, source_id))
                .collect();

            (None, errors)
        }
    }
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<Tokens, Vec<E>> {
    let stream = Stream::from_iter(source.chars());
    let result = lexer().parse(stream).into_result();

    match result {
        Ok(tokens) => Ok(Tokens(insert_start(tokens.to_vec()))),
        Err(errors) => {
            // Convert chumsky Simple errors to our Error type
            let errors = errors
                .into_iter()
                .map(|error| convert_lexer_error(&error, 0))
                .collect();

            Err(errors)
        }
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
pub fn lexer<'a>() -> impl Parser<'a, ParserInput<'a>, Vec<Token>, ParserError<'a>> {
    lex_token()
        .repeated()
        .collect()
        .then_ignore(ignored())
        .then_ignore(end())
}

/// Lex chars to a single token
fn lex_token<'a>() -> impl Parser<'a, ParserInput<'a>, Token, ParserError<'a>> {
    // Handle range token with proper whitespace
    // Ranges need special handling since the '..' token needs to know about whitespace
    // for binding on left and right sides
    let range = whitespace()
        .or_not()
        .then(just(".."))
        .then(whitespace().or_not())
        .map_with(|((left, _), right), extra| {
            let span: chumsky_0_10::span::SimpleSpan = extra.span();
            Token {
                kind: TokenKind::Range {
                    // Check if there was whitespace before/after to determine binding
                    // This maintains compatibility with the chumsky_0_9 implementation
                    bind_left: left.is_none(),
                    bind_right: right.is_none(),
                },
                span: span.start()..span.end(),
            }
        });

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
fn token<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    // Main token parser for all tokens
    choice((
        line_wrap(),                      // Line continuation with backslash
        newline().to(TokenKind::NewLine), // Newline characters
        multi_char_operators(),           // Multi-character operators (==, !=, etc.)
        interpolation(),                  // String interpolation (f"...", s"...")
        param(),                          // Parameters ($name)
        // Date literals must come before @ handling for annotations
        date_token(), // Date literals (@2022-01-01)
        // Special handling for @ annotations - must come after date_token
        just('@').to(TokenKind::Annotate), // @ annotation marker
        one_of("></%=+-*[]().,:|!{}").map(TokenKind::Control), // Single-character controls
        literal().map(TokenKind::Literal), // Literals (numbers, strings, etc.)
        keyword(),                         // Keywords (let, func, etc.)
        ident_part().map(TokenKind::Ident), // Identifiers
        comment(),                         // Comments (# and #!)
    ))
}

fn multi_char_operators<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    choice((
        just("->").to(TokenKind::ArrowThin),
        just("=>").to(TokenKind::ArrowFat),
        just("==").to(TokenKind::Eq),
        just("!=").to(TokenKind::Ne),
        just(">=").to(TokenKind::Gte),
        just("<=").to(TokenKind::Lte),
        just("~=").to(TokenKind::RegexSearch),
        just("&&").then_ignore(end_expr()).to(TokenKind::And),
        just("||").then_ignore(end_expr()).to(TokenKind::Or),
        just("??").to(TokenKind::Coalesce),
        just("//").to(TokenKind::DivInt),
        just("**").to(TokenKind::Pow),
    ))
}

fn keyword<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
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

fn param<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    just('$')
        .ignore_then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.')
                .repeated()
                .collect::<String>(),
        )
        .map(TokenKind::Param)
}

fn interpolation<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    // For s-strings and f-strings, we need to handle both regular and triple-quoted variants
    one_of("sf")
        .then(
            // Use a custom quoted_string implementation that better handles triple quotes
            choice((
                // Triple quote strings for s-strings
                just('"')
                    .then(just('"'))
                    .then(just('"'))
                    .ignore_then(any().filter(|&c| c != '"').repeated().collect::<String>())
                    .then_ignore(just('"').then(just('"')).then(just('"'))),
                // Regular quoted string
                quoted_string(true),
            )),
        )
        .map(|(c, s)| TokenKind::Interpolation(c, s))
}

fn ignored<'a>() -> impl Parser<'a, ParserInput<'a>, (), ParserError<'a>> {
    whitespace().repeated().ignored()
}

fn whitespace<'a>() -> impl Parser<'a, ParserInput<'a>, (), ParserError<'a>> {
    any()
        .filter(|x: &char| *x == ' ' || *x == '\t')
        .repeated()
        .at_least(1)
        .ignored()
}

// Custom newline parser for Stream<char>
fn newline<'a>() -> impl Parser<'a, ParserInput<'a>, (), ParserError<'a>> {
    just('\n')
        .or(just('\r').then_ignore(just('\n').or_not()))
        .ignored()
}

fn line_wrap<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
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

fn comment<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    // Extract the common comment text parser
    let comment_text = any()
        .filter(|c: &char| *c != '\n' && *c != '\r')
        .repeated()
        .collect::<String>();

    just('#').ignore_then(
        // One option would be to check that doc comments have new lines in the
        // lexer (we currently do in the parser); which would give better error
        // messages?
        just('!')
            .ignore_then(comment_text.clone().map(TokenKind::DocComment))
            .or(comment_text.map(TokenKind::Comment)),
    )
}

pub fn ident_part<'a>() -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
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
fn digits<'a>(count: usize) -> impl Parser<'a, ParserInput<'a>, Vec<char>, ParserError<'a>> {
    any()
        .filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
        .collect::<Vec<char>>()
}

fn date_inner<'a>() -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
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

fn time_inner<'a>() -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
    // Helper function for parsing time components with separators
    fn time_component<'p>(
        separator: char,
        component_parser: impl Parser<'p, ParserInput<'p>, Vec<char>, ParserError<'p>>,
    ) -> impl Parser<'p, ParserInput<'p>, String, ParserError<'p>> {
        just(separator)
            .then(component_parser)
            .map(move |(sep, comp)| format!("{}{}", sep, String::from_iter(comp)))
            .or_not()
            .map(|opt| opt.unwrap_or_default())
    }

    // Hours (required)
    let hours = digits(2).map(String::from_iter);

    // Minutes and seconds (optional) - with colon separator
    let minutes = time_component(':', digits(2));
    let seconds = time_component(':', digits(2));

    // Milliseconds (optional) - with dot separator
    let milliseconds = time_component(
        '.',
        any()
            .filter(|c: &char| c.is_ascii_digit())
            .repeated()
            .at_least(1)
            .at_most(6)
            .collect::<Vec<char>>(),
    );

    // Timezone (optional): either 'Z' or '+/-HH:MM'
    let timezone = choice((
        just('Z').map(|c| c.to_string()),
        one_of("-+")
            .then(digits(2).then(just(':').or_not().then(digits(2))).map(
                |(hrs, (_opt_colon, mins))| {
                    // Always format as -0800 without colon for SQL compatibility, regardless of input format
                    // We need to handle both -08:00 and -0800 input formats but standardize the output
                    format!("{}{}", String::from_iter(hrs), String::from_iter(mins))
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

fn date_token<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
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

pub fn literal<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
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
fn parse_number_with_base<'a>(
    prefix: &'static str,
    base: u32,
    max_digits: usize,
    valid_digit: impl Fn(&char) -> bool + 'a,
) -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
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

fn binary_number<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    parse_number_with_base("0b", 2, 32, |c| *c == '0' || *c == '1')
}

fn hexadecimal_number<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    parse_number_with_base("0x", 16, 12, |c| c.is_ascii_hexdigit())
}

fn octal_number<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    parse_number_with_base("0o", 8, 12, |c| ('0'..='7').contains(c))
}

fn number<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    // Helper function to build a string from optional number components
    fn optional_component<'p, T>(
        parser: impl Parser<'p, ParserInput<'p>, T, ParserError<'p>>,
        to_string: impl Fn(T) -> String + 'p,
    ) -> impl Parser<'p, ParserInput<'p>, String, ParserError<'p>> {
        parser
            .map(to_string)
            .or_not()
            .map(|opt| opt.unwrap_or_default())
    }

    // Parse integer part
    let integer = parse_integer().map(|chars| chars.into_iter().collect::<String>());

    // Parse fractional part
    let fraction_digits = any()
        .filter(|c: &char| c.is_ascii_digit())
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
        });

    let frac = just('.')
        .then(fraction_digits)
        .map(|(dot, digits)| format!("{}{}", dot, String::from_iter(digits)));

    // Parse exponent
    let exp_digits = one_of("+-")
        .or_not()
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .repeated()
                .at_least(1)
                .collect::<Vec<char>>(),
        )
        .map(|(sign_opt, digits)| {
            let mut s = String::new();
            if let Some(sign) = sign_opt {
                s.push(sign);
            }
            s.push_str(&String::from_iter(digits));
            s
        });

    let exp = one_of("eE")
        .then(exp_digits)
        .map(|(e, digits)| format!("{}{}", e, digits));

    // Combine all parts into a number using the helper function
    integer
        .then(optional_component(frac, |f| f))
        .then(optional_component(exp, |e| e))
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

fn parse_integer<'a>() -> impl Parser<'a, ParserInput<'a>, Vec<char>, ParserError<'a>> {
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
            // TODO: there's a few of these, which seems unlikely to be the
            // idomatic approach. I tried `.to_slice()` but couldn't get it to work
            .map(|(first, rest)| {
                let mut chars = vec![first];
                chars.extend(rest);
                chars
            }),
        just('0').map(|c| vec![c]),
    ))
}

fn string<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    quoted_string(true).map(Literal::String)
}

fn raw_string<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
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

fn boolean<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    choice((just("true").to(true), just("false").to(false)))
        .then_ignore(end_expr())
        .map(Literal::Boolean)
}

fn null<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
    just("null").to(Literal::Null).then_ignore(end_expr())
}

fn value_and_unit<'a>() -> impl Parser<'a, ParserInput<'a>, Literal, ParserError<'a>> {
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

pub fn quoted_string<'a>(
    escaped: bool,
) -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
    choice((
        quoted_triple_string(escaped),
        quoted_string_of_quote(&'"', escaped, false),
        quoted_string_of_quote(&'\'', escaped, false),
    ))
    .map(|chars| chars.into_iter().collect())
}

fn quoted_triple_string<'a>(
    _escaped: bool, // Not used in this implementation
) -> impl Parser<'a, ParserInput<'a>, Vec<char>, ParserError<'a>> {
    // Helper function to create triple quoted string parsers
    fn triple_quoted_parser<'p>(
        quote: char,
    ) -> impl Parser<'p, ParserInput<'p>, Vec<char>, ParserError<'p>> {
        let triple_quote_open = just(quote).then(just(quote)).then(just(quote));
        let triple_quote_close = just(quote).then(just(quote)).then(just(quote));

        triple_quote_open
            .ignore_then(
                // Keep consuming characters until we hit three quotes in a row
                any()
                    .filter(move |&c| c != quote)
                    .repeated()
                    .collect::<Vec<char>>(),
            )
            .then_ignore(triple_quote_close)
    }

    // Parser for triple quoted strings (both single and double quotes)
    choice((triple_quoted_parser('"'), triple_quoted_parser('\'')))
}

// TODO: not working, need to figure out how to convert the `then_with` in 0.9 to 0.10
//
// The commented code below shows how the 0.9 lexer handled multi-level quoted strings
// by counting the number of opening quotes and then creating a closing delimiter
// with the same count:
//
// fn quoted_string_of_quote2(
//     quote: &char,
//     escaping: bool,
// ) -> impl Parser<'_, ParserInput<'_>, Vec<char>, ParserError<'_>> {
//     let opening = just(*quote).repeated().at_least(1);
//
//     opening.then_with_ctx(move |opening| {
//         if opening.len() % 2 == 0 {
//             // If we have an even number of quotes, it's an empty string.
//             return (just(vec![])).boxed();
//         }
//         let delimiter = just(*quote).repeated().exactly(opening.len());
//
//         let inner = if escaping {
//             choice((
//                 // If we're escaping, don't allow consuming a backslash
//                 // We need the `vec` to satisfy the type checker
//                 (delimiter.or(just(vec!['\\']))).not(),
//                 escaped_character(),
//                 // Or escape the quote char of the current string
//                 just('\\').ignore_then(just(*quote)),
//             ))
//             .boxed()
//         } else {
//             delimiter.not().boxed()
//         };
//
//         inner.repeated().then_ignore(delimiter).boxed()
//     })
// }

fn quoted_string_of_quote(
    quote: &char,
    escaping: bool,
    allow_multiline: bool,
) -> impl Parser<'_, ParserInput<'_>, Vec<char>, ParserError<'_>> {
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
        just('\\').ignore_then(just(q)),            // Escaped quote
        just('\\').ignore_then(just('\\')),         // Escaped backslash
        just('\\').ignore_then(just('n')).to('\n'), // Newline
        just('\\').ignore_then(just('r')).to('\r'), // Carriage return
        just('\\').ignore_then(just('t')).to('\t'), // Tab
        escaped_character(),                        // Handle all other escape sequences
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

fn escaped_character<'a>() -> impl Parser<'a, ParserInput<'a>, char, ParserError<'a>> {
    just('\\').ignore_then(choice((
        just('\\'),
        just('/'),
        just('b').to('\x08'),
        just('f').to('\x0C'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
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

fn end_expr<'a>() -> impl Parser<'a, ParserInput<'a>, (), ParserError<'a>> {
    choice((
        end(),
        one_of(",)]}\t >").to(()),
        newline(),
        just("..").to(()),
    ))
    .rewind()
}
