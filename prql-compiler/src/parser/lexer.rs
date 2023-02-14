use chumsky::prelude::*;

use crate::ast::pl::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    NewLine,

    Ident(String),
    Keyword(String),
    Literal(Literal),

    Interpolation(char, String),

    // this contains 3 bytes at most, we should replace it with SmallStr
    Control(String),
}

pub fn lexer() -> impl Parser<char, Vec<(Token, std::ops::Range<usize>)>, Error = Simple<char>> {
    let new_line = just('\r').or_not().then(just('\n')).to(Token::NewLine);

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
        just(".."),
    ))
    .map(|x| x.to_string())
    .map(Token::Control);

    let control = one_of("></%=+-*[]().,:|!")
        .map(|c: char| c.to_string())
        .map(Token::Control);

    let ident = ident_part().map(Token::Ident);

    let keyword = choice((just("func"), just("let"), just("switch"), just("prql")))
        .then_ignore(end_expr())
        .map(|x| x.to_string())
        .map(Token::Keyword);

    let literal = literal().map(Token::Literal);

    // s-string and f-strings
    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| Token::Interpolation(c, s));

    let token = choice((
        new_line.clone(),
        control_multi,
        interpolation,
        control,
        literal,
        keyword,
        ident,
    ))
    .recover_with(skip_then_retry_until([]).skip_start());

    let whitespace = just('\t').or(just(' ')).repeated().at_least(1).ignored();
    let comment = just('#').then(none_of('\n').repeated());
    let comments = comment
        .separated_by(new_line.then(whitespace.or_not()))
        .at_least(1)
        .ignored();

    token
        .map_with_span(|tok, span| (tok, span))
        .padded_by(comments.or(whitespace).repeated())
        .repeated()
        .then_ignore(end())
}

pub fn ident_part() -> impl Parser<char, String, Error = Simple<char>> {
    let plain = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_' || *c == '$')
        .map(Some)
        .chain::<char, Vec<_>, _>(
            filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated(),
        )
        .collect();

    let backticks = just('`')
        .ignore_then(none_of('`').repeated())
        .then_ignore(just('`'))
        .collect::<String>();

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
        .try_map(|chars, span| {
            let str = chars.into_iter().filter(|c| *c != '_').collect::<String>();

            if let Ok(i) = str.parse::<i64>() {
                Ok(Literal::Integer(i))
            } else if let Ok(f) = str.parse::<f64>() {
                Ok(Literal::Float(f))
            } else {
                Err(Simple::custom(span, "invalid number"))
            }
        })
        .labelled("number");

    let string = quoted_string(true).map(Literal::String);

    let raw_string = just("r")
        .ignore_then(quoted_string(false))
        .map(Literal::String);

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
                Err(Simple::custom(span, "invalid number"))
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
        .collect::<String>()
        .map(Literal::Date);

    let time = just('@')
        .ignore_then(time_inner.clone())
        .then_ignore(end_expr())
        .collect::<String>()
        .map(Literal::Time);

    let datetime = just('@')
        .ignore_then(date_inner)
        .chain(just('T'))
        .chain::<char, _, _>(time_inner)
        .then_ignore(end_expr())
        .collect::<String>()
        .map(Literal::Timestamp);

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

fn quoted_string(escaped: bool) -> impl Parser<char, String, Error = Simple<char>> {
    choice((
        quoted_string_inner(r#"""""#, escaped),
        quoted_string_inner(r#"'''"#, escaped),
        quoted_string_inner(r#"""#, escaped),
        quoted_string_inner(r#"'"#, escaped),
    ))
    .collect::<String>()
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
        .or(one_of(",)]\n\t ").ignored())
        .or(just("..").ignored())
        .rewind()
}

impl Token {
    pub fn ctrl<S: ToString>(s: S) -> Self {
        Token::Control(s.to_string())
    }
}

#[deny(clippy::derive_hash_xor_eq)]
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
            Self::Ident(arg0) => {
                if arg0.is_empty() {
                    write!(f, "an identifier")
                } else {
                    write!(f, "`{arg0}`")
                }
            }
            Self::Keyword(arg0) => write!(f, "keyword {arg0}"),
            Self::Literal(arg0) => write!(f, "{arg0}"),
            Self::Control(arg0) => write!(f, "{arg0}"),
            Self::Interpolation(c, s) => {
                write!(f, "{c}\"{}\"", s)
            }
        }
    }
}
