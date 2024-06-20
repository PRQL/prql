use insta::assert_debug_snapshot;
use insta::assert_snapshot;

use crate::lexer::lr::token::{Literal, TokenKind};
use crate::lexer::TokenVec;
use crate::lexer::{lexer, literal, quoted_string};

#[test]
fn line_wrap() {
    assert_debug_snapshot!(TokenVec(lexer().parse(r"5 +
    \ 3 "
        ).unwrap()), @r###"
        TokenVec(
            [
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                3..9: LineWrap([]),
                10..11: Literal(Integer(3)),
            ],
        )
        "###);

    // Comments are included; no newline after the comments
    assert_debug_snapshot!(TokenVec(lexer().parse(r"5 +
# comment
   # comment with whitespace
  \ 3 "
        ).unwrap()), @r###"
        TokenVec(
            [
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                3..46: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
                47..48: Literal(Integer(3)),
            ],
        )
        "###);

    // Check display, for the test coverage (use `assert_eq` because the
    // line-break doesn't work well with snapshots)
    assert_eq!(
        format!(
            "{}",
            TokenKind::LineWrap(vec![TokenKind::Comment(" a comment".to_string())])
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
        TokenVec(
            [
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                4..5: Literal(Integer(3)),
            ],
        )
        "###);
}

#[test]
fn comment() {
    assert_debug_snapshot!(TokenVec(lexer().parse("# comment\n# second line").unwrap()), @r###"
        TokenVec(
            [
                0..9: Comment(" comment"),
                9..10: NewLine,
                10..23: Comment(" second line"),
            ],
        )
        "###);

    assert_snapshot!(TokenKind::Comment(" This is a single-line comment".to_string()), @r###"
        # This is a single-line comment
        "###);
}

#[test]
fn doc_comment() {
    assert_debug_snapshot!(TokenVec(lexer().parse("#! docs").unwrap()), @r###"
        TokenVec(
            [
                0..7: DocComment(" docs"),
            ],
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
        TokenVec(
            [
                0..1: Literal(Integer(1)),
                1..3: Range { bind_left: true, bind_right: true },
                3..4: Literal(Integer(2)),
            ],
        )
        "###);

    assert_debug_snapshot!(TokenVec(lexer().parse("..2").unwrap()), @r###"
        TokenVec(
            [
                0..2: Range { bind_left: true, bind_right: true },
                2..3: Literal(Integer(2)),
            ],
        )
        "###);

    assert_debug_snapshot!(TokenVec(lexer().parse("1..").unwrap()), @r###"
        TokenVec(
            [
                0..1: Literal(Integer(1)),
                1..3: Range { bind_left: true, bind_right: true },
            ],
        )
        "###);

    assert_debug_snapshot!(TokenVec(lexer().parse("in ..5").unwrap()), @r###"
        TokenVec(
            [
                0..2: Ident("in"),
                2..5: Range { bind_left: false, bind_right: true },
                5..6: Literal(Integer(5)),
            ],
        )
        "###);
}

#[test]
fn test_lex_source() {
    use insta::assert_debug_snapshot;

    assert_debug_snapshot!(lex_source("5 + 3"), @r###"
    Ok(
        TokenVec(
            [
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                4..5: Literal(Integer(3)),
            ],
        ),
    )
    "###);

    // Something that will generate an error
    assert_debug_snapshot!(lex_source("^"), @r###"
    Err(
        [
            Error {
                kind: Error,
                span: Some(
                    0:0-1,
                ),
                reason: Unexpected {
                    found: "^",
                },
                hints: [],
                code: None,
            },
        ],
    )
    "###);
}
