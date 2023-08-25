use chumsky::{error::Cheap, prelude::*};
use prql_ast::expr::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    NewLine,

    Ident(String),
    Keyword(String),
    Literal(Literal),
    Param(String),

    Range {
        bind_left: bool,
        bind_right: bool,
    },
    Interpolation(char, String),

    /// single-char control tokens
    Control(char),

    ArrowThin,   // ->
    ArrowFat,    // =>
    Eq,          // ==
    Ne,          // !=
    Gte,         // >=
    Lte,         // <=
    RegexSearch, // ~=
    And,         // &&
    Or,          // ||
    Coalesce,    // ??
    DivInt,      // //
    Annotate,    // @
}

pub fn lexer() -> impl Parser<char, Vec<(Token, std::ops::Range<usize>)>, Error = Cheap<char>> {
    let control_multi = choice((
        just("->").to(Token::ArrowThin),
        just("=>").to(Token::ArrowFat),
        just("==").to(Token::Eq),
        just("!=").to(Token::Ne),
        just(">=").to(Token::Gte),
        just("<=").to(Token::Lte),
        just("~=").to(Token::RegexSearch),
        just("&&").then_ignore(end_expr()).to(Token::And),
        just("||").then_ignore(end_expr()).to(Token::Or),
        just("??").to(Token::Coalesce),
        just("//").to(Token::DivInt),
        just("@").then(digits(1).not().rewind()).to(Token::Annotate),
    ));

    let control = one_of("></%=+-*[]().,:|!{}").map(Token::Control);

    let ident = ident_part().map(Token::Ident);

    let keyword = choice((
        just("let"),
        just("into"),
        just("case"),
        just("prql"),
        just("type"),
        just("module"),
        just("internal"),
        just("func"),
    ))
    .then_ignore(end_expr())
    .map(|x| x.to_string())
    .map(Token::Keyword);

    let literal = literal().map(Token::Literal);

    let param = just('$')
        .ignore_then(filter(|c: &char| c.is_alphanumeric() || *c == '_' || *c == '.').repeated())
        .collect::<String>()
        .map(Token::Param);

    // s-string and f-strings
    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| Token::Interpolation(c, s));

    let token = choice((
        new_line(),
        control_multi,
        interpolation,
        param,
        control,
        literal,
        keyword,
        ident,
    ))
    .recover_with(skip_then_retry_until([]).skip_start());

    let whitespace = whitespace();
    let range = (whitespace.clone().or_not())
        .then_ignore(just(".."))
        .then(whitespace.or_not())
        .map(|(left, right)| Token::Range {
            bind_left: left.is_none(),
            bind_right: right.is_none(),
        })
        .map_with_span(|tok, span| (tok, span));

    // range needs to consume leading whitespace,
    // so whitespace following a token must not be consumed
    comment()
        .or_not()
        .ignore_then(choice((
            range,
            ignored().ignore_then(token.map_with_span(|tok, span| (tok, span))),
        )))
        .repeated()
        .then_ignore(ignored())
        .then_ignore(end())
}

pub fn ident_part() -> impl Parser<char, String, Error = Cheap<char>> {
    let plain = filter(|c: &char| c.is_alphabetic() || *c == '_')
        .map(Some)
        .chain::<char, Vec<_>, _>(filter(|c: &char| c.is_alphanumeric() || *c == '_').repeated())
        .collect();

    let backticks = just('`')
        .ignore_then(none_of('`').repeated())
        .then_ignore(just('`'))
        .collect::<String>();

    plain.or(backticks)
}

fn new_line() -> impl Parser<char, Token, Error = Cheap<char>> {
    just('\n').to(Token::NewLine)
}

fn whitespace() -> impl Parser<char, (), Error = Cheap<char>> + Clone {
    one_of("\t \r").repeated().at_least(1).ignored()
}

fn comment() -> impl Parser<char, (), Error = Cheap<char>> {
    let comment = just('#').then(none_of('\n').repeated());

    comment
        .separated_by(new_line().then(whitespace().or_not()))
        .at_least(1)
        .ignored()
}

fn ignored() -> impl Parser<char, (), Error = Cheap<char>> {
    comment()
        .or(whitespace())
        .or(line_continuation())
        .repeated()
        .ignored()
}

fn line_continuation() -> impl Parser<char, (), Error = Cheap<char>> {
    just('\n')
        .then(whitespace().repeated().or_not())
        .then(just('\\'))
        .ignored()
}

fn literal() -> impl Parser<char, Literal, Error = Cheap<char>> {
    let exp = just('e').or(just('E')).chain(
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
                Err(Cheap::expected_input_found(span, None, None))
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
            one_of("-+")
                .chain(
                    // TODO: This is repeated without the `:`~ with an `or`
                    // because using `.or_not` triggers a request for
                    // type hints, which seems difficult to provide... Is there
                    // an easier way?
                    //
                    //   (digits(2).chain(just(':').or_not()).chain(digits(2)))
                    //
                    (digits(2).chain(just(':')).chain(digits(2)))
                        .or(digits(2).chain(digits(2)))
                        .or(just('Z').map(|x| vec![x])),
                )
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
        (just('u').ignore_then(
            filter(|c: &char| c.is_ascii_hexdigit())
                .repeated()
                .exactly(4)
                .collect::<String>()
                .validate(|digits, span, emit| {
                    char::from_u32(u32::from_str_radix(&digits, 16).unwrap()).unwrap_or_else(|| {
                        emit(Cheap::expected_input_found(span, None, None));
                        '\u{FFFD}' // unicode replacement character
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
        one_of(",)]}\r\n\t >").ignored(),
        just("..").ignored(),
    ))
    .rewind()
}

impl Token {
    pub fn range(bind_left: bool, bind_right: bool) -> Self {
        Token::Range {
            bind_left,
            bind_right,
        }
    }
}

// This is here because Literal::Float(f64) does not implement Hash, so we cannot simply derive it.
// There are reasons for that, but chumsky::Error needs Hash for the Token, so it can deduplicate
// tokens in error.
// So this hack could lead to duplicated tokens in error messages. Oh no.
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
    }
}

impl std::cmp::Eq for Token {}

#[test]
fn test_line_continuation() {
    use insta::assert_debug_snapshot;

    line_continuation()
        .then_ignore(end())
        .parse(
            r#"
\"#,
        )
        .unwrap();

    line_continuation()
        .then_ignore(end())
        .parse(
            r#"
    \"#,
        )
        .unwrap();

    // (TODO: is there a terser way of writing our lexer output?)
    assert_debug_snapshot!(lexer().parse(r#"5 +
    \ 3 "#
        ).unwrap(), @r###"
    [
        (
            Literal(
                Integer(
                    5,
                ),
            ),
            0..1,
        ),
        (
            Control(
                '+',
            ),
            2..3,
        ),
        (
            Literal(
                Integer(
                    3,
                ),
            ),
            10..11,
        ),
    ]
    "###);

    // TODO: this is how a comment appears — with a newline
    assert_debug_snapshot!(lexer().parse(r#"5 +
    # comment
    \ 3 "#
        ).unwrap(), @r###"
    [
        (
            Literal(
                Integer(
                    5,
                ),
            ),
            0..1,
        ),
        (
            Control(
                '+',
            ),
            2..3,
        ),
        (
            NewLine,
            3..4,
        ),
        (
            Literal(
                Integer(
                    3,
                ),
            ),
            24..25,
        ),
    ]
    "###);
}

#[test]
fn quotes() {
    use insta::assert_snapshot;

    // All these are valid & equal.
    assert_snapshot!(quoted_string(false).parse(r#"'aoeu'"#).unwrap(), @"aoeu");
    assert_snapshot!(quoted_string(false).parse(r#"'''aoeu'''"#).unwrap(), @"aoeu");
    assert_snapshot!(quoted_string(false).parse(r#"'''''aoeu'''''"#).unwrap(), @"aoeu");
    assert_snapshot!(quoted_string(false).parse(r#"'''''''aoeu'''''''"#).unwrap(), @"aoeu");

    // An even number is interpreted as a closed string (and the remainder is unparsed)
    assert_snapshot!(quoted_string(false).parse(r#"''aoeu''"#).unwrap(), @"");

    // When not escaping, we take the inner string between the three quotes
    assert_snapshot!(quoted_string(false).parse(r#""""\"hello\""""#).unwrap(), @r###"\"hello\"###);

    assert_snapshot!(quoted_string(true).parse(r#""""\"hello\"""""#).unwrap(), @r###""hello""###);

    // Escape each inner quote depending on the outer quote
    assert_snapshot!(quoted_string(true).parse(r#""\"hello\"""#).unwrap(), @r###""hello""###);
    assert_snapshot!(quoted_string(true).parse(r#"'\'hello\''"#).unwrap(), @"'hello'");

    assert_snapshot!(quoted_string(true).parse(r#"''"#).unwrap(), @"");

    // An empty input should fail
    quoted_string(false).parse(r#""#).unwrap_err();

    // An even number of quotes is an empty string
    assert_snapshot!(quoted_string(true).parse(r#"''''''"#).unwrap(), @"");
}
