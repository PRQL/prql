use chumsky::prelude::*;

use crate::ast::pl::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    NewLine,

    Ident,
    Keyword,
    Literal,

    Interpolation,

    // this contains 3 bytes at most, we should replace it with SmallStr
    Control,
}

pub fn lexer() -> impl Parser<char, Vec<(Token, std::ops::Range<usize>)>, Error = Simple<char>> {
    let new_line = just('\r').or_not().then(just('\n')).to(Token::NewLine);
    let whitespace = just('\t').or(just(' ')).repeated().at_least(1).ignored();

    let control_multi = choice((
        just("->"),
        just("=>"),
        just("=="),
        just("!="),
        just(">="),
        just("<="),
        just("and").then_ignore(end_expr()),
        just("or").then_ignore(end_expr()),
        just("??"),
    ))
    .to(Token::Control);

    let control = one_of("></%=+-*[]().,:|!").to(Token::Control);

    let ident = ident_part().to(Token::Ident);

    let keyword = choice((just("func"), just("let"), just("switch"), just("prql")))
        .then_ignore(end_expr())
        .to(Token::Keyword);

    let literal = literal().to(Token::Literal);

    // s-string and f-strings
    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .to(Token::Interpolation);

    let token = choice((
        new_line.clone(),
        control_multi,
        interpolation,
        control,
        literal,
        keyword,
        ident,
    ));

    let comment = just('#').then(none_of('\n').repeated());
    let comments = comment
        .separated_by(new_line.then(whitespace.or_not()))
        .at_least(1)
        .ignored();

    // range needs to consume leading whitespace,
    // so whitespace following a token must not be consumed
    let ignored = comments.or(whitespace).repeated();

    token
        .map_with_span(|tok, span| (tok, span))
        .padded_by(ignored)
        .repeated()
        .then_ignore(end())
}

pub fn ident_part() -> impl Parser<char, (), Error = Simple<char>> {
    let plain = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_' || *c == '$')
        .map(Some)
        .chain::<char, Vec<_>, _>(
            filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated(),
        )
        .ignored();

    let backticks = just('`')
        .ignore_then(none_of('`').repeated())
        .then_ignore(just('`'))
        .ignored();

    plain.or(backticks)
}

fn literal() -> impl Parser<char, Literal, Error = Simple<char>> {
    let exp = just('e').or(just('E')).ignore_then(
        just('+')
            .or(just('-'))
            .or_not()
            .chain::<char, _, _>(text::digits(10)),
    );

    let integer = filter(|c: &char| c.is_ascii_digit() && *c != '0')
        .chain::<_, Vec<char>, _>(filter(|c: &char| c.is_ascii_digit() || *c == '_').repeated())
        .or(just('0').map(|c| vec![c]));

    let frac = just('.')
        .chain::<char, _, _>(filter(|c: &char| c.is_ascii_digit()))
        .chain::<char, _, _>(filter(|c: &char| c.is_ascii_digit() || *c == '_').repeated());

    let number = just('+')
        .or(just('-'))
        .or_not()
        .chain::<char, _, _>(integer)
        .chain::<char, _, _>(frac.or_not().flatten())
        .chain::<char, _, _>(exp.or_not().flatten())
        .try_map(|chars, span| Ok(Literal::Null))
        .labelled("number");

    let string = quoted_string(true).to(Literal::Null);

    let raw_string = just("r")
        .ignore_then(quoted_string(false))
        .to(Literal::Null);

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
        .to(Literal::Null);

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
            one_of("-+")
                .chain(
                    digits(2)
                        .chain(just(':'))
                        .chain(digits(2))
                        .or(just('Z').map(|x| vec![x])),
                )
                .or_not()
                .flatten(),
        )
        .boxed();

    let date = just('@')
        .ignore_then(date_inner.clone())
        .then_ignore(end_expr())
        .to(Literal::Null);

    let time = just('@')
        .ignore_then(time_inner.clone())
        .then_ignore(end_expr())
        .to(Literal::Null);

    let datetime = just('@')
        .ignore_then(date_inner)
        .chain(just('T'))
        .chain::<char, _, _>(time_inner)
        .then_ignore(end_expr())
        .to(Literal::Null);

    choice((
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

fn quoted_string(escaped: bool) -> impl Parser<char, (), Error = Simple<char>> {
    // I don't know how this could be simplified and implemented for n>3 in general
    choice((
        quoted_string_inner(r#""""""""#, escaped),
        quoted_string_inner(r#"""""""#, escaped),
        quoted_string_inner(r#""""""#, escaped),
        quoted_string_inner(r#"""""#, escaped),
        quoted_string_inner(r#"""#, escaped),
        quoted_string_inner(r#"''''''"#, escaped),
        quoted_string_inner(r#"'''''"#, escaped),
        quoted_string_inner(r#"''''"#, escaped),
        quoted_string_inner(r#"'''"#, escaped),
        quoted_string_inner(r#"'"#, escaped),
    ))
    .ignored()
    .labelled("string")
}

fn quoted_string_inner(
    quotes: &str,
    escaping: bool,
) -> impl Parser<char, Vec<char>, Error = Simple<char>> + '_ {
    let mut forbidden = just(quotes).boxed();

    if escaping {
        forbidden = just(quotes).or(just("\\")).boxed()
    };

    let mut inner = forbidden.not().boxed();

    if escaping {
        inner = inner
            .or(just('\\').ignore_then(
                just('\\')
                    .or(just('/'))
                    .or(just('"'))
                    .or(just('b').to('\x08'))
                    .or(just('f').to('\x0C'))
                    .or(just('n').to('\n'))
                    .or(just('r').to('\r'))
                    .or(just('t').to('\t'))
                    .or(just('u').ignore_then(
                        filter(|c: &char| c.is_ascii_hexdigit())
                            .repeated()
                            .exactly(4)
                            .collect::<String>()
                            .validate(|digits, span, emit| {
                                char::from_u32(u32::from_str_radix(&digits, 16).unwrap())
                                    .unwrap_or_else(|| {
                                        emit(Simple::custom(span, "invalid unicode character"));
                                        '\u{FFFD}' // unicode replacement character
                                    })
                            }),
                    )),
            ))
            .boxed();
    }

    inner.repeated().delimited_by(just(quotes), just(quotes))
}

fn digits(count: usize) -> impl Parser<char, Vec<char>, Error = Simple<char>> {
    filter(|c: &char| c.is_ascii_digit())
        .repeated()
        .exactly(count)
}

fn end_expr() -> impl Parser<char, (), Error = Simple<char>> {
    end()
        .or(one_of(",)]\r\n\t ").ignored())
        .or(just("..").ignored())
        .rewind()
}

impl Token {
    pub fn ctrl<S: ToString>(s: S) -> Self {
        Token::Control
    }
}

// This is here because Literal::Float(f64) does not implement Hash, so we cannot simply derive it.
// There are reasons for that, but chumsky::Error needs Hash for the Token, so it can deduplicate
// tokens in error.
// So this hack could lead to duplicated tokens in error messages. Oh no.
#[allow(clippy::derive_hash_xor_eq)]
impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl std::cmp::Eq for Token {}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewLine => write!(f, "new line"),
            Self::Ident => {
                write!(f, "an identifier")
            }
            Self::Keyword => write!(f, "keyword"),
            Self::Literal => write!(f, "literal"),
            Self::Control => write!(f, "control"),
            Self::Interpolation => {
                write!(f, "Interpolation")
            }
        }
    }
}
