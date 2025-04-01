/*
# Implementation Plan for Chumsky 0.10.0 Lexer

## 1. Core API Changes to Address

1. **Parser Trait Changes**:
   - Update signature to accommodate new lifetime parameter
   - Adjust for the new `I` parameter semantics (entire input vs token type)
   - Move appropriate operations to use the new `IterParser` trait

2. **Combinator Replacements**:
   - Replace `take_until()` with combinations of `any()`, `and_is()`, and `not()`
   - Update any usage of `chain()` with appropriate alternatives
   - Add explicit type annotations where needed due to less type inference

3. **Error Handling**:
   - Update error types from `error::Cheap` to the new error system
   - Modify error conversion functions to work with the new error types

## 2. Implementation Steps

### Phase 1: Initial Setup (Already Done)
- ✅ Create feature flag structure
- ✅ Set up parallel module for 0.10 implementation
- ✅ Create stub functions for the new lexer

### Phase 2: Core Lexer Functions (Current Phase)
1. ✅ Implement basic token parsers:
   - Minimal implementations of the token parsers
   - Stub functions for test-only methods
   - Set up proper error handling infrastructure

2. ✅ Update the main lexer function:
   - Implement minimally functional lex_source() and lex_source_recovery()
   - Set up error handling structure

3. 🔄 Refactor into combinators (In Progress):
   - Split up the big function into separate parser combinators
   - Structure for chumsky 0.10 compatibility
   - Ensure proper interfaces and function signatures

### Phase 3: Complex Parsers (Next Phase)
1. Refactor overall structure:
   - Update parser function signatures to work with chumsky 0.10
   - Refine error handling approach
   - Setup the core lexer infrastructure

2. Reimplement basic token parsers:
   - Control characters, single and multi-character
   - Identifiers and keywords
   - Simple literals (boolean, null)
   - Comments and whitespace handling

3. Reimplement complex parsers:
   - String literals with proper handling of escape sequences
   - Numeric literals (integers, floats, hex, octal, etc.)
   - Date and time literals
   - Special tokens (ranges, parameters, etc.)

### Phase 4: Optimization and Testing
1. Apply performance optimizations:
   - Take advantage of the new optimization capabilities
   - Consider using the new `regex` combinator where appropriate

2. Build comprehensive tests:
   - Ensure all token types are recognized correctly
   - Compare outputs with the 0.9 implementation
   - Test error reporting with various malformed inputs

### Phase 5: Integration and Finalization
1. Remove any compatibility shims
2. Document key differences and approaches
3. Update any dependent code to work with the new lexer

## 3. Specific Migration Notes

### Parser Combinator Migrations
- `filter` → `filter` (likely similar usage but verify signature)
- `just` → `just` (verify signature)
- `choice` → `choice` (verify signature)
- `then_ignore(end())` → may no longer be needed
- `repeated()` → May need to use from `IterParser` trait
- `map_with_span` → Verify how span handling has changed

### Error Handling
- Replace `Cheap<char>` with appropriate error type
- Update error conversion to handle the new error type structure
- Ensure error spans are correctly propagated

### Additional Recommendations
- Take advantage of new features like regex parsing for simple patterns
- Consider using the new Pratt parser for any expression parsing
- The new eager evaluation model may change behavior - test thoroughly
- Use the improved zero-copy capabilities where appropriate

### Resources

Check out these issues for more details:
- https://github.com/zesterer/chumsky/issues/747
- https://github.com/zesterer/chumsky/issues/745
- https://github.com/zesterer/chumsky/releases/tag/0.10

### Tests
- After each group of changes, run:
   ```
   # cargo check for this module
   cargo check -p prqlc-parser --features chumsky-10 -- chumsky_0_10

   # tests for this module
   cargo insta test --accept -p prqlc-parser --features chumsky-10 -- chumsky_0_10

   # confirm the existing tests still pass without this feature
   cargo insta test -p prqlc-parser
   ```
- and the linting instructions in `CLAUDE.md`

*/

use chumsky_0_10::error::Rich;
use chumsky_0_10::input::{Input, Stream};
use chumsky_0_10::prelude::*;
use chumsky_0_10::text::newline;

use super::lr::{Literal, Token, TokenKind, Tokens, ValueAndUnit};
use crate::error::{Error, ErrorSource, Reason, WithErrorInfo};
use crate::span::Span;

type E = Error;

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, source_id: u16) -> (Option<Vec<Token>>, Vec<E>) {
    let stream = Stream::from_iter(source_id as usize..source_id as usize + 1, source.chars());

    match lexer().parse(stream) {
        Ok(tokens) => (Some(insert_start(tokens)), vec![]),
        Err(errors) => {
            let errors = errors
                .into_iter()
                .map(|e| convert_lexer_error(source, e, source_id))
                .collect();
            (None, errors)
        }
    }
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<Tokens, Vec<E>> {
    let stream = Stream::from_iter(0..1, source.chars());

    lexer()
        .parse(stream)
        .map(insert_start)
        .map(Tokens)
        .map_err(|errors| {
            errors
                .into_iter()
                .map(|e| convert_lexer_error(source, e, 0))
                .collect()
        })
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

fn convert_lexer_error(source: &str, e: Rich<char, SimpleSpan>, source_id: u16) -> Error {
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
        .with_source(ErrorSource::Lexer(format!("{:?}", e)))
}

/// Lex chars to tokens until the end of the input
pub(crate) fn lexer<'src>(
) -> impl Parser<'src, impl Input<'src> + Clone, Vec<Token>, Error = Rich<'src, char>> {
    lex_token()
        .repeated()
        .collect()
        .then_ignore(ignored())
        .then_ignore(end())
}

/// Lex chars to a single token
fn lex_token<'src>() -> impl Parser<'src, impl Input<'src> + Clone, Token, Error = Rich<'src, char>>
{
    let control_multi = choice((
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
        just("@")
            .then(digits(1).not().rewind())
            .to(TokenKind::Annotate),
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

    let param = just('$')
        .ignore_then(
            filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.')
                .repeated()
                .collect::<String>(),
        )
        .map(TokenKind::Param);

    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| TokenKind::Interpolation(c, s));

    let token = choice((
        line_wrap(),
        newline().to(TokenKind::NewLine),
        control_multi,
        interpolation,
        param,
        control,
        literal,
        keyword,
        ident,
        comment(),
    ))
    .recover_with(skip_then_retry_until([]));

    let range = (whitespace().or_not())
        .then_ignore(just(".."))
        .then(whitespace().or_not())
        .map(|(left, right)| TokenKind::Range {
            // If there was no whitespace before (after), then we mark the range
            // as bound on the left (right).
            bind_left: left.is_none(),
            bind_right: right.is_none(),
        })
        .map_with_span(|kind, span| Token {
            kind,
            span: span.into(),
        });

    choice((
        range,
        ignored().ignore_then(token.map_with_span(|kind, span| Token {
            kind,
            span: span.into(),
        })),
    ))
}

fn ignored<'src>() -> impl Parser<'src, impl Input<'src> + Clone, (), Error = Rich<'src, char>> {
    whitespace().repeated().ignored()
}

fn whitespace<'src>() -> impl Parser<'src, impl Input<'src> + Clone, (), Error = Rich<'src, char>> {
    filter(|x: &char| *x == ' ' || *x == '\t')
        .repeated()
        .at_least(1)
        .ignored()
}

fn line_wrap<'src>(
) -> impl Parser<'src, impl Input<'src> + Clone, TokenKind, Error = Rich<'src, char>> {
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

fn comment<'src>(
) -> impl Parser<'src, impl Input<'src> + Clone, TokenKind, Error = Rich<'src, char>> {
    just('#').ignore_then(choice((
        // One option would be to check that doc comments have new lines in the
        // lexer (we currently do in the parser); which would give better error
        // messages?
        just('!').ignore_then(
            take_until(newline())
                .map(|(chars, _)| chars.into_iter().collect::<String>())
                .map(TokenKind::DocComment),
        ),
        take_until(newline())
            .map(|(chars, _)| chars.into_iter().collect::<String>())
            .map(TokenKind::Comment),
    )))
}

pub(crate) fn ident_part<'src>(
) -> impl Parser<'src, impl Input<'src> + Clone, String, Error = Rich<'src, char>> + Clone {
    let plain = filter(|c: &char| c.is_alphabetic() || *c == '_')
        .chain(filter(|c: &char| c.is_alphanumeric() || *c == '_').repeated());

    let backticks = none_of('`').repeated().delimited_by(just('`'), just('`'));

    plain.or(backticks).collect()
}

fn literal<'src>() -> impl Parser<'src, impl Input<'src> + Clone, Literal, Error = Rich<'src, char>>
{
    let binary_notation = just("0b")
        .then_ignore(just("_").or_not())
        .ignore_then(
            filter(|c: &char| *c == '0' || *c == '1')
                .repeated()
                .at_least(1)
                .at_most(32)
                .collect::<String>()
                .try_map(|digits, span| {
                    i64::from_str_radix(&digits, 2)
                        .map(Literal::Integer)
                        .map_err(|_| Rich::custom(span, "Invalid binary number"))
                }),
        )
        .labelled("number");

    let hexadecimal_notation = just("0x")
        .then_ignore(just("_").or_not())
        .ignore_then(
            filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .at_least(1)
                .at_most(12)
                .collect::<String>()
                .try_map(|digits, span| {
                    i64::from_str_radix(&digits, 16)
                        .map(Literal::Integer)
                        .map_err(|_| Rich::custom(span, "Invalid hexadecimal number"))
                }),
        )
        .labelled("number");

    let octal_notation = just("0o")
        .then_ignore(just("_").or_not())
        .ignore_then(
            filter(|&c| ('0'..='7').contains(&c))
                .repeated()
                .at_least(1)
                .at_most(12)
                .collect::<String>()
                .try_map(|digits, span| {
                    i64::from_str_radix(&digits, 8)
                        .map(Literal::Integer)
                        .map_err(|_| Rich::custom(span, "Invalid octal number"))
                }),
        )
        .labelled("number");

    let exp = one_of("eE").chain(one_of("+-").or_not().chain(text::digits(10)));

    let integer = filter(|c: &char| c.is_ascii_digit() && *c != '0')
        .chain::<_, Vec<char>, _>(filter(|c: &char| c.is_ascii_digit() || *c == '_').repeated())
        .or(just('0').map(|c| vec![c]));

    let frac = just('.')
        .chain::<char, _, _>(filter(|c: &char| c.is_ascii_digit()))
        .chain::<char, _, _>(filter(|c: &char| c.is_ascii_digit() || *c == '_').repeated());

    let number = integer
        .chain::<char, _, _>(frac.or_not().flatten())
        .chain::<char, _, _>(exp.or_not().flatten())
        .try_map(|chars, span| {
            let str = chars.into_iter().filter(|c| *c != '_').collect::<String>();

            if let Ok(i) = str.parse::<i64>() {
                Ok(Literal::Integer(i))
            } else if let Ok(f) = str.parse::<f64>() {
                Ok(Literal::Float(f))
            } else {
                Err(Rich::custom(span, "Invalid number"))
            }
        })
        .labelled("number");

    let string = quoted_string(true).map(Literal::String);

    let raw_string = just("r")
        .ignore_then(quoted_string(false))
        .map(Literal::RawString);

    let bool = (just("true").to(true))
        .or(just("false").to(false))
        .then_ignore(end_expr())
        .map(Literal::Boolean);

    let null = just("null").to(Literal::Null).then_ignore(end_expr());

    let value_and_unit = integer
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
        .try_map(|(number, unit), span| {
            let str = number.into_iter().filter(|c| *c != '_').collect::<String>();
            if let Ok(n) = str.parse::<i64>() {
                let unit = unit.to_string();
                Ok(ValueAndUnit { n, unit })
            } else {
                Err(Rich::custom(span, "Invalid number for duration"))
            }
        })
        .map(Literal::ValueAndUnit);

    let date_inner = digits(4)
        .chain(just('-'))
        .chain::<char, _, _>(digits(2))
        .chain::<char, _, _>(just('-'))
        .chain::<char, _, _>(digits(2))
        .boxed();

    let time_inner = digits(2)
        // minutes
        .chain::<char, _, _>(just(':').chain(digits(2)).or_not().flatten())
        // seconds
        .chain::<char, _, _>(just(':').chain(digits(2)).or_not().flatten())
        // milliseconds
        .chain::<char, _, _>(
            just('.')
                .chain(
                    filter(|c: &char| c.is_ascii_digit())
                        .repeated()
                        .at_least(1)
                        .at_most(6),
                )
                .or_not()
                .flatten(),
        )
        // timezone offset
        .chain::<char, _, _>(
            choice((
                // Either just `Z`
                just('Z').map(|x| vec![x]),
                // Or an offset, such as `-05:00` or `-0500`
                one_of("-+").chain(
                    digits(2)
                        .then_ignore(just(':').or_not())
                        .chain::<char, _, _>(digits(2)),
                ),
            ))
            .or_not()
            .flatten(),
        )
        .boxed();

    // Not an annotation
    let dt_prefix = just('@').then(just('{').not().rewind());

    let date = dt_prefix
        .ignore_then(date_inner.clone())
        .then_ignore(end_expr())
        .collect::<String>()
        .map(Literal::Date);

    let time = dt_prefix
        .ignore_then(time_inner.clone())
        .then_ignore(end_expr())
        .collect::<String>()
        .map(Literal::Time);

    let datetime = dt_prefix
        .ignore_then(date_inner)
        .chain(just('T'))
        .chain::<char, _, _>(time_inner)
        .then_ignore(end_expr())
        .collect::<String>()
        .map(Literal::Timestamp);

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
        datetime,
        date,
        time,
    ))
}

fn quoted_string<'src>(
    escaped: bool,
) -> impl Parser<'src, impl Input<'src> + Clone, String, Error = Rich<'src, char>> {
    choice((
        quoted_string_of_quote(&'"', escaped),
        quoted_string_of_quote(&'\'', escaped),
    ))
    .collect::<String>()
    .labelled("string")
}

fn quoted_string_of_quote<'src, 'a>(
    quote: &'a char,
    escaping: bool,
) -> impl Parser<'src, impl Input<'src> + Clone, Vec<char>, Error = Rich<'src, char>> + 'a {
    let opening = just(*quote).repeated().at_least(1);

    opening.then_with(move |opening| {
        if opening.len() % 2 == 0 {
            // If we have an even number of quotes, it's an empty string.
            return empty().to(vec![]).boxed();
        }
        let delimiter = just(*quote).repeated().exactly(opening.len());

        let inner = if escaping {
            choice((
                // If we're escaping, don't allow consuming a backslash
                // We need the `vec` to satisfy the type checker
                not(delimiter.clone().or(just('\\').to(()))).to(()),
                escaped_character(),
                // Or escape the quote char of the current string
                just('\\').ignore_then(just(*quote)),
            ))
            .boxed()
        } else {
            not(delimiter.clone()).to(()).boxed()
        };

        any()
            .and_is(inner)
            .repeated()
            .then_ignore(delimiter)
            .boxed()
    })
}

fn escaped_character<'src>(
) -> impl Parser<'src, impl Input<'src> + Clone, char, Error = Rich<'src, char>> {
    just('\\').ignore_then(choice((
        just('\\'),
        just('/'),
        just('b').to('\x08'),
        just('f').to('\x0C'),
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        (just("u{").ignore_then(
            filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .at_least(1)
                .at_most(6)
                .collect::<String>()
                .try_map(|digits, span| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap())
                        .ok_or_else(|| Rich::custom(span, "Invalid unicode character"))
                })
                .then_ignore(just('}')),
        )),
        (just('x').ignore_then(
            filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(2)
                .collect::<String>()
                .try_map(|digits, span| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap())
                        .ok_or_else(|| Rich::custom(span, "Invalid character escape"))
                }),
        )),
    )))
}

fn digits<'src>(
    count: usize,
) -> impl Parser<'src, impl Input<'src> + Clone, Vec<char>, Error = Rich<'src, char>> {
    filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
}

fn end_expr<'src>() -> impl Parser<'src, impl Input<'src> + Clone, (), Error = Rich<'src, char>> {
    choice((
        end(),
        one_of(",)]}\t >").to(()),
        newline().to(()),
        just("..").to(()),
    ))
    .rewind()
}
