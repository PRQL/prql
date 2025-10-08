//! PRQL Lexer implementation

use chumsky_0_10 as chumsky;

use chumsky::extra;
use chumsky::prelude::*;
use chumsky::Parser;

use self::lr::{Literal, Token, TokenKind, Tokens, ValueAndUnit};
use crate::error::{Error, ErrorSource, Reason, WithErrorInfo};

pub mod lr;
#[cfg(test)]
mod test;

type E = Error;
type ParserInput<'a> = &'a str;
type ParserError<'a> = extra::Err<Simple<'a, char>>;

/// Convert a chumsky Simple error to our internal Error type
fn convert_lexer_error(source: &str, error: &Simple<'_, char>, source_id: u16) -> E {
    // Get span information from the Simple error
    // NOTE: When parsing &str, SimpleSpan uses BYTE offsets, not character offsets!
    // We need to convert byte offsets to character offsets for compatibility with our error reporting.
    let byte_span = error.span();
    let byte_start = byte_span.start();
    let byte_end = byte_span.end();

    // Convert byte offsets to character offsets
    let char_start = source[..byte_start].chars().count();
    let char_end = source[..byte_end].chars().count();

    // Extract the "found" text using character-based slicing
    let found: String = source
        .chars()
        .skip(char_start)
        .take(char_end - char_start)
        .collect();

    // If found is empty, report as "end of input", otherwise wrap in quotes
    let found_display = if found.is_empty() {
        "end of input".to_string()
    } else {
        format!("'{}'", found)
    };

    // Create a new Error with the extracted information
    let error_source = format!(
        "Unexpected {} at position {}..{}",
        found_display, char_start, char_end
    );

    Error::new(Reason::Unexpected {
        found: found_display,
    })
    .with_span(Some(crate::span::Span {
        start: char_start,
        end: char_end,
        source_id,
    }))
    .with_source(ErrorSource::Lexer(error_source))
}

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, source_id: u16) -> (Option<Vec<Token>>, Vec<E>) {
    let result = lexer().parse(source).into_result();

    match result {
        Ok(tokens) => (Some(insert_start(tokens.to_vec())), vec![]),
        Err(errors) => {
            // Convert chumsky Simple errors to our Error type
            let errors = errors
                .into_iter()
                .map(|error| convert_lexer_error(source, &error, source_id))
                .collect();

            (None, errors)
        }
    }
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<Tokens, Vec<E>> {
    let result = lexer().parse(source).into_result();

    match result {
        Ok(tokens) => Ok(Tokens(insert_start(tokens.to_vec()))),
        Err(errors) => {
            // Convert chumsky Simple errors to our Error type
            let errors = errors
                .into_iter()
                .map(|error| convert_lexer_error(source, &error, 0))
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
        .then_ignore(whitespace().or_not())
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
            let span: chumsky::span::SimpleSpan = extra.span();
            Token {
                kind: TokenKind::Range {
                    // Check if there was whitespace before/after to determine binding
                    bind_left: left.is_none(),
                    bind_right: right.is_none(),
                },
                span: span.start()..span.end(),
            }
        });

    // Handle all other token types with proper whitespace
    let other_tokens = whitespace()
        .or_not()
        .ignore_then(token().map_with(|kind, extra| {
            let span: chumsky::span::SimpleSpan = extra.span();
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
    .to_slice()
    .then_ignore(end_expr())
    .map(|s: &str| TokenKind::Keyword(s.to_string()))
}

fn param<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    just('$')
        .ignore_then(
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.')
                .repeated()
                .to_slice()
                .map(|s: &str| s.to_string()),
        )
        .map(TokenKind::Param)
}

fn interpolation<'a>() -> impl Parser<'a, ParserInput<'a>, TokenKind, ParserError<'a>> {
    // For s-strings and f-strings, use the same multi-quote string parser
    // No escaping for interpolated strings
    //
    // NOTE: Known limitation in error reporting for unclosed interpolated strings:
    // When an f-string or s-string is unclosed (e.g., `f"{}`), the error is reported at the
    // opening quote position (e.g., position 17) rather than at the end of input where the
    // closing quote should be (e.g., position 20). This is because the `.then()` combinator
    // modifies error spans during error recovery, and there's no way to prevent this from
    // custom parsers.
    one_of("sf")
        .then(quoted_string(false))
        .map(|(c, s)| TokenKind::Interpolation(c, s))
}

fn whitespace<'a>() -> impl Parser<'a, ParserInput<'a>, (), ParserError<'a>> {
    text::inline_whitespace().at_least(1)
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
    let comment_text = none_of("\n\r").repeated().collect::<String>();

    just('#').ignore_then(
        // One option would be to check that doc comments have new lines in the
        // lexer (we currently do in the parser); which would give better error
        // messages?
        just('!')
            .ignore_then(comment_text.map(TokenKind::DocComment))
            .or(comment_text.map(TokenKind::Comment)),
    )
}

pub fn ident_part<'a>() -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
    let plain = any()
        .filter(|c: &char| c.is_alphabetic() || *c == '_')
        .then(
            // this could _almost_ just be, but we don't currently allow numbers
            // (should we?)
            //
            // .then(text::ascii::ident())
            any()
                .filter(|c: &char| c.is_alphanumeric() || *c == '_')
                .repeated(),
        )
        .to_slice()
        .map(|s: &str| s.to_string());

    let backtick = none_of('`')
        .repeated()
        .collect::<String>()
        .delimited_by(just('`'), just('`'));

    choice((plain, backtick))
}

// Date/time components
fn digits<'a>(count: usize) -> impl Parser<'a, ParserInput<'a>, Vec<char>, ParserError<'a>> {
    chumsky::text::digits(10)
        .exactly(count)
        .collect::<Vec<char>>()
}

fn date_inner<'a>() -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
    // Format: YYYY-MM-DD
    text::digits(10)
        .exactly(4)
        .then(just('-'))
        .then(text::digits(10).exactly(2))
        .then(just('-'))
        .then(text::digits(10).exactly(2))
        .to_slice()
        // TODO: can change this to return the slice and avoid the allocation
        .map(|s: &str| s.to_owned())
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
    let integer = parse_integer();

    // Parse fractional part
    let fraction_digits = any()
        .filter(|c: &char| c.is_ascii_digit())
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit() || *c == '_')
                .repeated(),
        )
        .to_slice();

    let frac = just('.')
        .then(fraction_digits)
        .map(|(dot, digits): (char, &str)| format!("{}{}", dot, digits));

    // Parse exponent
    let exp_digits = one_of("+-")
        .or_not()
        .then(
            any()
                .filter(|c: &char| c.is_ascii_digit())
                .repeated()
                .at_least(1),
        )
        .to_slice();

    let exp = one_of("eE")
        .then(exp_digits)
        .map(|(e, digits): (char, &str)| format!("{}{}", e, digits));

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

fn parse_integer<'a>() -> impl Parser<'a, ParserInput<'a>, &'a str, ParserError<'a>> {
    // Handle both multi-digit numbers (can't start with 0) and single digit 0
    choice((
        any()
            .filter(|c: &char| c.is_ascii_digit() && *c != '0')
            .then(
                any()
                    .filter(|c: &char| c.is_ascii_digit() || *c == '_')
                    .repeated(),
            )
            .to_slice(),
        just('0').to_slice(),
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
    parse_integer().then(unit).then_ignore(end_expr()).map(
        |(number_str, unit_str): (&str, &str)| {
            // Parse the number (removing underscores), defaulting to 1 if parsing fails
            let n = number_str.replace('_', "").parse::<i64>().unwrap_or(1);
            Literal::ValueAndUnit(ValueAndUnit {
                n,
                unit: unit_str.to_string(),
            })
        },
    )
}

pub fn quoted_string<'a>(
    escaped: bool,
) -> impl Parser<'a, ParserInput<'a>, String, ParserError<'a>> {
    choice((
        multi_quoted_string(&'"', escaped),
        multi_quoted_string(&'\'', escaped),
    ))
    .map(|chars| chars.into_iter().collect())
}

// Helper function to parse escape sequences
// Takes the input and the quote character, returns the escaped character
fn parse_escape_sequence<'a>(
    input: &mut chumsky::input::InputRef<'a, '_, ParserInput<'a>, ParserError<'a>>,
    quote_char: char,
) -> char {
    match input.peek() {
        Some(next_ch) => {
            input.next();
            match next_ch {
                '\\' => '\\',
                '/' => '/',
                'b' => '\x08',
                'f' => '\x0C',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'u' if input.peek() == Some('{') => {
                    input.next(); // consume '{'
                    let mut hex = String::new();
                    while let Some(ch) = input.peek() {
                        if ch == '}' {
                            input.next();
                            break;
                        }
                        if ch.is_ascii_hexdigit() && hex.len() < 6 {
                            hex.push(ch);
                            input.next();
                        } else {
                            break;
                        }
                    }
                    char::from_u32(u32::from_str_radix(&hex, 16).unwrap_or(0)).unwrap_or('\u{FFFD}')
                }
                'x' => {
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(ch) = input.peek() {
                            if ch.is_ascii_hexdigit() {
                                hex.push(ch);
                                input.next();
                            }
                        }
                    }
                    if hex.len() == 2 {
                        char::from_u32(u32::from_str_radix(&hex, 16).unwrap_or(0))
                            .unwrap_or('\u{FFFD}')
                    } else {
                        next_ch // Just use the character after backslash
                    }
                }
                c if c == quote_char => quote_char, // Escaped quote
                other => other,                     // Unknown escape, keep the character
            }
        }
        None => {
            // Backslash at end of input
            '\\'
        }
    }
}

// Implementation of multi-level quoted strings using custom parser
// Handles odd number of quotes (1, 3, 5, etc.) for strings with content
// and even number of quotes (2, 4, 6, etc.) for empty strings
//
// This uses a single custom parser that dynamically handles arbitrary quote counts
// All quoted strings allow newlines
fn multi_quoted_string<'a>(
    quote: &char,
    escaping: bool,
) -> impl Parser<'a, ParserInput<'a>, Vec<char>, ParserError<'a>> {
    let quote_char = *quote;

    custom(move |input| {
        let start_cursor = input.save();

        // Count opening quotes
        let mut open_count = 0;
        while let Some(ch) = input.peek() {
            if ch == quote_char {
                input.next();
                open_count += 1;
            } else {
                break;
            }
        }

        if open_count == 0 {
            let span = input.span_since(start_cursor.cursor());
            return Err(Simple::new(input.peek_maybe(), span));
        }

        // Even number of quotes -> empty string
        if open_count % 2 == 0 {
            return Ok(vec![]);
        }

        // Odd number of quotes -> parse content until we find the closing delimiter
        let mut result = Vec::new();

        loop {
            // Save position to potentially rewind
            let checkpoint = input.save();

            // Try to match the closing delimiter (open_count quotes)
            let mut close_count = 0;
            while close_count < open_count {
                match input.peek() {
                    Some(ch) if ch == quote_char => {
                        input.next();
                        close_count += 1;
                    }
                    _ => break,
                }
            }

            // If we matched the full delimiter, we're done
            if close_count == open_count {
                return Ok(result);
            }

            // Not the delimiter - rewind and consume one content character
            input.rewind(checkpoint);

            match input.next() {
                Some(ch) => {
                    // Handle escape sequences if escaping is enabled
                    if escaping && ch == '\\' {
                        let escaped = parse_escape_sequence(input, quote_char);
                        result.push(escaped);
                    } else {
                        result.push(ch);
                    }
                }
                None => {
                    // Can't find closing delimiter - return error about unclosed string
                    // Create a zero-width span at the current position (end of input)
                    let current_cursor = input.save();
                    let span = input.span_since(current_cursor.cursor());
                    return Err(Simple::new(None, span));
                }
            }
        }
    })
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
