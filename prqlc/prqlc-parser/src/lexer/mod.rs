use chumsky::error::Cheap;
use chumsky::prelude::*;
use chumsky::text::{newline, Character};

use self::lr::{Literal, Token, TokenKind, ValueAndUnit};
use crate::error::{Error, ErrorSource, Reason, WithErrorInfo};
use crate::span::Span;

pub mod lr;
#[cfg(test)]
mod test;

/// Lex PRQL into LR, returning both the LR and any errors encountered
pub fn lex_source_recovery(source: &str, source_id: u16) -> (Option<Vec<Token>>, Vec<Error>) {
    let (tokens, lex_errors) = lexer().parse_recovery(source);

    let tokens = tokens.map(insert_start);

    let errors = lex_errors
        .into_iter()
        .map(|e| convert_lexer_error(source, e, source_id))
        .collect();

    log::debug!("lex errors: {:?}", errors);
    (tokens, errors)
}

/// Lex PRQL into LR, returning either the LR or the errors encountered
pub fn lex_source(source: &str) -> Result<lr::Tokens, Vec<Error>> {
    lexer()
        .parse(source)
        .map(insert_start)
        .map(lr::Tokens)
        .map_err(|e| {
            e.into_iter()
                .map(|x| convert_lexer_error(source, x, 0))
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

/// Lex chars to tokens until the end of the input
pub(crate) fn lexer() -> impl Parser<char, Vec<Token>, Error = Cheap<char>> {
    lex_token()
        .repeated()
        .then_ignore(ignored())
        .then_ignore(end())
}

/// Lex chars to a single token
fn lex_token() -> impl Parser<char, Token, Error = Cheap<char>> {
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
        .ignore_then(filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.').repeated())
        .collect::<String>()
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
    .recover_with(skip_then_retry_until([]).skip_start());

    let range = (whitespace().or_not())
        .then_ignore(just(".."))
        .then(whitespace().or_not())
        .map(|(left, right)| TokenKind::Range {
            // If there was no whitespace before (after), then we mark the range
            // as bound on the left (right).
            bind_left: left.is_none(),
            bind_right: right.is_none(),
        })
        .map_with_span(|kind, span| Token { kind, span });

    choice((
        range,
        ignored().ignore_then(token.map_with_span(|kind, span| Token { kind, span })),
    ))
}

fn ignored() -> impl Parser<char, (), Error = Cheap<char>> {
    whitespace().repeated().ignored()
}

fn whitespace() -> impl Parser<char, (), Error = Cheap<char>> {
    filter(|x: &char| x.is_inline_whitespace())
        .repeated()
        .at_least(1)
        .ignored()
}

fn line_wrap() -> impl Parser<char, TokenKind, Error = Cheap<char>> {
    newline()
        .ignore_then(
            whitespace()
                .repeated()
                .ignore_then(comment())
                .then_ignore(newline())
                .repeated(),
        )
        .then_ignore(whitespace().repeated())
        .then_ignore(just('\\'))
        .map(TokenKind::LineWrap)
}

fn comment() -> impl Parser<char, TokenKind, Error = Cheap<char>> {
    just('#').ignore_then(choice((
        // One option would be to check that doc comments have new lines in the
        // lexer (we currently do in the parser); which would give better error
        // messages?
        just('!').ignore_then(
            newline()
                .not()
                .repeated()
                .collect::<String>()
                .map(TokenKind::DocComment),
        ),
        newline()
            .not()
            .repeated()
            .collect::<String>()
            .map(TokenKind::Comment),
    )))
}

pub(crate) fn ident_part() -> impl Parser<char, String, Error = Cheap<char>> + Clone {
    let plain = filter(|c: &char| c.is_alphabetic() || *c == '_')
        .chain(filter(|c: &char| c.is_alphanumeric() || *c == '_').repeated());

    let backticks = none_of('`').repeated().delimited_by(just('`'), just('`'));

    plain.or(backticks).collect()
}

fn literal() -> impl Parser<char, Literal, Error = Cheap<char>> {
    let binary_notation = just("0b")
        .then_ignore(just("_").or_not())
        .ignore_then(
            filter(|c: &char| *c == '0' || *c == '1')
                .repeated()
                .at_least(1)
                .at_most(32)
                .collect::<String>()
                .try_map(|digits, _| {
                    Ok(Literal::Integer(i64::from_str_radix(&digits, 2).unwrap()))
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
                .try_map(|digits, _| {
                    Ok(Literal::Integer(i64::from_str_radix(&digits, 16).unwrap()))
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
                .try_map(|digits, _| {
                    Ok(Literal::Integer(i64::from_str_radix(&digits, 8).unwrap()))
                }),
        )
        .labelled("number");

    let exp = one_of("eE").chain(one_of("+-").or_not().chain::<char, _, _>(text::digits(10)));

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
                Err(Cheap::expected_input_found(span, None, None))
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
                Err(Cheap::expected_input_found(span, None, None))
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
            .or_not(),
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

fn quoted_string(escaped: bool) -> impl Parser<char, String, Error = Cheap<char>> {
    choice((
        quoted_string_of_quote(&'"', escaped),
        quoted_string_of_quote(&'\'', escaped),
    ))
    .collect::<String>()
    .labelled("string")
}

fn quoted_string_of_quote(
    quote: &char,
    escaping: bool,
) -> impl Parser<char, Vec<char>, Error = Cheap<char>> + '_ {
    let opening = just(*quote).repeated().at_least(1);

    opening.then_with(move |opening| {
        if opening.len() % 2 == 0 {
            // If we have an even number of quotes, it's an empty string.
            return (just(vec![])).boxed();
        }
        let delimiter = just(*quote).repeated().exactly(opening.len());

        let inner = if escaping {
            choice((
                // If we're escaping, don't allow consuming a backslash
                // We need the `vec` to satisfy the type checker
                (delimiter.or(just(vec!['\\']))).not(),
                escaped_character(),
                // Or escape the quote char of the current string
                just('\\').ignore_then(just(*quote)),
            ))
            .boxed()
        } else {
            delimiter.not().boxed()
        };

        inner.repeated().then_ignore(delimiter).boxed()
    })
}

fn escaped_character() -> impl Parser<char, char, Error = Cheap<char>> {
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
                .validate(|digits, span, emit| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap_or_else(|| {
                        emit(Cheap::expected_input_found(span, None, None));
                        '\u{FFFD}' // Unicode replacement character
                    })
                })
                .then_ignore(just('}')),
        )),
        (just('x').ignore_then(
            filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(2)
                .collect::<String>()
                .validate(|digits, span, emit| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap_or_else(|| {
                        emit(Cheap::expected_input_found(span, None, None));
                        '\u{FFFD}'
                    })
                }),
        )),
    )))
}

fn digits(count: usize) -> impl Parser<char, Vec<char>, Error = Cheap<char>> {
    filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
}

fn end_expr() -> impl Parser<char, (), Error = Cheap<char>> {
    choice((
        end(),
        one_of(",)]}\t >").ignored(),
        newline(),
        just("..").ignored(),
    ))
    .rewind()
}
