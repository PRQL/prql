#![allow(dead_code)]

use chumsky::prelude::*;

use crate::ast::pl::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    Whitespace,
    NewLine,

    Ident(String),
    Literal(Literal),

    // this contains 3 bytes at most, we should replace it with SmallStr
    Control(String),
}

pub fn lexer() -> impl Parser<char, Vec<(Token, std::ops::Range<usize>)>, Error = Simple<char>> {
    let whitespace = just('\t')
        .or(just(' '))
        .repeated()
        .at_least(1)
        .to(Token::Whitespace);

    let new_line = just('\r').or_not().then(just('\n')).to(Token::NewLine);

    let control_multi = just("->")
        .or(just("=>"))
        .or(just("=="))
        .or(just("!="))
        .or(just(">="))
        .or(just("<="))
        .or(just("and")) // TODO: negative lookahead for whitespace
        .or(just("or")) // TODO: negative lookahead for whitespace
        .or(just("??"))
        .map(|x| x.to_string())
        .map(Token::Control);

    let control = one_of("></%=+-*[]().,:|")
        .map(|c: char| c.to_string())
        .map(Token::Control);

    let ident = ident_part().map(Token::Ident);

    let literal = literal().map(Token::Literal);

    let comment = just('#').then(filter(|c: &char| *c != '\n').repeated());

    whitespace
        .or(new_line)
        .or(control_multi)
        .or(control)
        .or(literal)
        .or(ident)
        .map_with_span(|tok, span| (tok, span))
        .padded_by(comment.repeated())
        .repeated()
        .then_ignore(end())
}

fn ident_part() -> impl Parser<char, String, Error = Simple<char>> {
    let plain = filter(|c: &char| c.is_ascii_alphabetic() || *c == '_' || *c == '$')
        .map(Some)
        .chain::<char, Vec<_>, _>(
            filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_').repeated(),
        )
        .collect();

    let backticks = just('`')
        .ignore_then(filter(|c| *c != '`').repeated())
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

    let number_part = filter(|c: &char| c.is_digit(10) && *c != '0')
        .chain::<_, Vec<char>, _>(filter(|c: &char| c.is_digit(10) || *c == '_').repeated())
        .collect()
        .or(just('0').map(|c| vec![c]));

    let frac = just('.').chain(number_part);

    let number = just('+')
        .or(just('-'))
        .or_not()
        .chain::<char, _, _>(number_part)
        .chain::<char, _, _>(frac.or_not().flatten())
        .chain::<char, _, _>(exp.or_not().flatten())
        .try_map(|chars, span| {
            // pest is responsible for ensuring these are in the correct place,
            // so we just need to remove them.
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

    let string = string();

    let bool = (just("true").to(true))
        .or(just("false").to(false))
        .map(Literal::Boolean);

    let null = just("null").to(Literal::Null);

    let value_and_unit = number_part
        .then(
            just("microseconds")
                .or(just("milliseconds"))
                .or(just("seconds"))
                .or(just("minutes"))
                .or(just("hours"))
                .or(just("days"))
                .or(just("weeks"))
                .or(just("months"))
                .or(just("years")),
        )
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

    // TODO: timestamp
    // TODO: date
    // TODO: time
    // TODO: "(" ~ literal ~ ")" }  --- should this even be here?

    string.or(number).or(bool).or(null).or(value_and_unit)
}

fn string() -> impl Parser<char, Literal, Error = Simple<char>> {
    let escape = just('\\').ignore_then(
        just('\\')
            .or(just('/'))
            .or(just('"'))
            .or(just('b').to('\x08'))
            .or(just('f').to('\x0C'))
            .or(just('n').to('\n'))
            .or(just('r').to('\r'))
            .or(just('t').to('\t'))
            .or(just('u').ignore_then(
                filter(|c: &char| c.is_digit(16))
                    .repeated()
                    .exactly(4)
                    .collect::<String>()
                    .validate(|digits, span, emit| {
                        char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap_or_else(
                            || {
                                emit(Simple::custom(span, "invalid unicode character"));
                                '\u{FFFD}' // unicode replacement character
                            },
                        )
                    }),
            )),
    );

    // TODO: multi-quoted strings (this is just parsing JSON strings)
    (just('\'')
        .ignore_then(filter(|c| *c != '\\' && *c != '\'').or(escape).repeated())
        .then_ignore(just('\'')))
    .or(just('"')
        .ignore_then(filter(|c| *c != '\\' && *c != '"').or(escape).repeated())
        .then_ignore(just('"')))
    .collect::<String>()
    .map(Literal::String)
    .labelled("string")
}

impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl std::cmp::Eq for Token {}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Whitespace => write!(f, "whitespace"),
            Self::NewLine => write!(f, "new line"),
            Self::Ident(arg0) => write!(f, "`{arg0}`"),
            Self::Literal(arg0) => write!(f, "{arg0}"),
            Self::Control(arg0) => write!(f, "{arg0}"),
        }
    }
}
