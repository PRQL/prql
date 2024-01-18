use chumsky::{
    error::Cheap,
    prelude::*,
    text::{newline, Character},
};

use prqlc_ast::expr::*;

#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    NewLine,

    Ident(String),
    Keyword(String),
    Literal(Literal),
    Param(String),

    Range {
        /// Whether the left side of the range is bound by the previous token
        /// (but it's not contained in this token)
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
    // Pow,         // **
    Annotate, // @

    // Aesthetics only
    Comment(String),
    DocComment(String),
    /// Vec contains comments between the newline and the line wrap
    // Currently we include the comments with the LineWrap token. This isn't
    // ideal, but I'm not sure of an easy way of having them be separate.
    // - The line wrap span technically include the comments — on a newline,
    //   we need to look ahead to _after_ the comments to see if there's a
    //   line wrap, and exclude the newline if there is.
    // - We can only pass one token back
    //
    // Alternatives:
    // - Post-process the stream, removing the newline prior to a line wrap.
    //   But requires a whole extra pass.
    // - Change the functionality. But it's very nice to be able to comment
    //   something out and have line-wraps still work.
    LineWrap(Vec<Token>),
}

/// Lex chars to tokens until the end of the input
pub fn lexer() -> impl Parser<char, Vec<TokenSpan>, Error = Cheap<char>> {
    lex_token()
        .repeated()
        .then_ignore(ignored())
        .then_ignore(end())
}

/// Lex chars to a single token
pub fn lex_token() -> impl Parser<char, TokenSpan, Error = Cheap<char>> {
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
        // just("**").to(Token::Pow),
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

    let interpolation = one_of("sf")
        .then(quoted_string(true))
        .map(|(c, s)| Token::Interpolation(c, s));

    let token = choice((
        line_wrap(),
        newline().to(Token::NewLine),
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
        .map(|(left, right)| Token::Range {
            // If there was no whitespace before (after), then we mark the range
            // as bound on the left (right).
            bind_left: left.is_none(),
            bind_right: right.is_none(),
        })
        .map_with_span(TokenSpan);

    choice((range, ignored().ignore_then(token.map_with_span(TokenSpan))))
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

fn line_wrap() -> impl Parser<char, Token, Error = Cheap<char>> {
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
        .map(Token::LineWrap)
}

fn comment() -> impl Parser<char, Token, Error = Cheap<char>> {
    just('#').ignore_then(choice((
        just('!').ignore_then(
            newline()
                .not()
                .repeated()
                .collect::<String>()
                .map(Token::DocComment),
        ),
        newline()
            .not()
            .repeated()
            .collect::<String>()
            .map(Token::Comment),
    )))
}

pub fn ident_part() -> impl Parser<char, String, Error = Cheap<char>> + Clone {
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

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::NewLine => write!(f, "new line"),
            Token::Ident(s) => {
                if s.is_empty() {
                    // FYI this shows up in errors
                    write!(f, "an identifier")
                } else {
                    write!(f, "{s}")
                }
            }
            Token::Keyword(s) => write!(f, "keyword {s}"),
            Token::Literal(lit) => write!(f, "{}", lit),
            Token::Control(c) => write!(f, "{c}"),

            Token::ArrowThin => f.write_str("->"),
            Token::ArrowFat => f.write_str("=>"),
            Token::Eq => f.write_str("=="),
            Token::Ne => f.write_str("!="),
            Token::Gte => f.write_str(">="),
            Token::Lte => f.write_str("<="),
            Token::RegexSearch => f.write_str("~="),
            Token::And => f.write_str("&&"),
            Token::Or => f.write_str("||"),
            Token::Coalesce => f.write_str("??"),
            Token::DivInt => f.write_str("//"),
            // Token::Pow => f.write_str("**"),
            Token::Annotate => f.write_str("@{"),

            Token::Param(id) => write!(f, "${id}"),

            Token::Range {
                bind_left,
                bind_right,
            } => write!(
                f,
                "'{}..{}'",
                if *bind_left { "" } else { " " },
                if *bind_right { "" } else { " " }
            ),
            Token::Interpolation(c, s) => {
                write!(f, "{c}\"{}\"", s)
            }
            Token::Comment(s) => {
                writeln!(f, "#{}", s)
            }
            Token::DocComment(s) => {
                writeln!(f, "#!{}", s)
            }
            Token::LineWrap(comments) => {
                write!(f, "\n\\ ")?;
                for comment in comments {
                    write!(f, "{}", comment)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TokenSpan(pub Token, pub std::ops::Range<usize>);

impl std::fmt::Debug for TokenSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}..{}: {:?}", self.1.start, self.1.end, self.0)
    }
}

pub struct TokenVec(pub Vec<TokenSpan>);

impl std::fmt::Debug for TokenVec {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "TokenVec (")?;
        for token in self.0.iter() {
            writeln!(f, "  {:?},", token)?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use insta::assert_debug_snapshot;
    use insta::assert_display_snapshot;
    use insta::assert_snapshot;

    #[test]
    fn line_wrap() {
        assert_debug_snapshot!(TokenVec(lexer().parse(r"5 +
    \ 3 "
        ).unwrap()), @r###"
        TokenVec (
          0..1: Literal(Integer(5)),
          2..3: Control('+'),
          3..9: LineWrap([]),
          10..11: Literal(Integer(3)),
        )
        "###);

        // Comments are included; no newline after the comments
        assert_debug_snapshot!(TokenVec(lexer().parse(r"5 +
# comment
   # comment with whitespace
  \ 3 "
        ).unwrap()), @r###"
        TokenVec (
          0..1: Literal(Integer(5)),
          2..3: Control('+'),
          3..46: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
          47..48: Literal(Integer(3)),
        )
        "###);

        // Check display, for the test coverage (use `assert_eq` because the
        // line-break doesn't work well with snapshots)
        assert_eq!(
            format!(
                "{}",
                Token::LineWrap(vec![Token::Comment(" a comment".to_string())])
            ),
            r#"
\ # a comment
"#
        );
    }

    #[test]
    fn numbers() {
        // Binary notation
        assert_eq!(
            literal().parse("0b1111000011110000").unwrap(),
            Literal::Integer(61680)
        );
        assert_eq!(
            literal().parse("0b_1111000011110000").unwrap(),
            Literal::Integer(61680)
        );

        // Hexadecimal notation
        assert_eq!(literal().parse("0xff").unwrap(), Literal::Integer(255));
        assert_eq!(
            literal().parse("0x_deadbeef").unwrap(),
            Literal::Integer(3735928559)
        );

        // Octal notation
        assert_eq!(literal().parse("0o777").unwrap(), Literal::Integer(511));
    }

    #[test]
    fn debug_display() {
        assert_debug_snapshot!(TokenVec(lexer().parse("5 + 3").unwrap()), @r###"
    TokenVec (
      0..1: Literal(Integer(5)),
      2..3: Control('+'),
      4..5: Literal(Integer(3)),
    )
    "###);
    }

    #[test]
    fn comment() {
        assert_debug_snapshot!(TokenVec(lexer().parse("# comment\n# second line").unwrap()), @r###"
        TokenVec (
          0..9: Comment(" comment"),
          9..10: NewLine,
          10..23: Comment(" second line"),
        )
        "###);

        assert_display_snapshot!(Token::Comment(" This is a single-line comment".to_string()), @r###"
        # This is a single-line comment
        "###);
    }

    #[test]
    fn doc_comment() {
        assert_debug_snapshot!(TokenVec(lexer().parse("#! docs").unwrap()), @r###"
        TokenVec (
          0..7: DocComment(" docs"),
        )
        "###);
    }

    #[test]
    fn quotes() {
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
        assert_snapshot!(quoted_string(true).parse(r"'\'hello\''").unwrap(), @"'hello'");

        assert_snapshot!(quoted_string(true).parse(r#"''"#).unwrap(), @"");

        // An empty input should fail
        quoted_string(false).parse(r#""#).unwrap_err();

        // An even number of quotes is an empty string
        assert_snapshot!(quoted_string(true).parse(r#"''''''"#).unwrap(), @"");

        // Hex escape
        assert_snapshot!(quoted_string(true).parse(r"'\x61\x62\x63'").unwrap(), @"abc");

        // Unicode escape
        assert_snapshot!(quoted_string(true).parse(r"'\u{01f422}'").unwrap(), @"🐢");
    }

    #[test]
    fn range() {
        assert_debug_snapshot!(TokenVec(lexer().parse("1..2").unwrap()), @r###"
        TokenVec (
          0..1: Literal(Integer(1)),
          1..3: Range { bind_left: true, bind_right: true },
          3..4: Literal(Integer(2)),
        )
        "###);

        assert_debug_snapshot!(TokenVec(lexer().parse("..2").unwrap()), @r###"
        TokenVec (
          0..2: Range { bind_left: true, bind_right: true },
          2..3: Literal(Integer(2)),
        )
        "###);

        assert_debug_snapshot!(TokenVec(lexer().parse("1..").unwrap()), @r###"
        TokenVec (
          0..1: Literal(Integer(1)),
          1..3: Range { bind_left: true, bind_right: true },
        )
        "###);

        assert_debug_snapshot!(TokenVec(lexer().parse("in ..5").unwrap()), @r###"
        TokenVec (
          0..2: Ident("in"),
          2..5: Range { bind_left: false, bind_right: true },
          5..6: Literal(Integer(5)),
        )
        "###);
    }
}
