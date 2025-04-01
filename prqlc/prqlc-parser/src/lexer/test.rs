#[cfg(not(feature = "chumsky-10"))]
use chumsky::Parser;

#[cfg(feature = "chumsky-10")]
use chumsky_0_10::Parser;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;

use crate::lexer::lr::{Literal, TokenKind, Tokens};
#[cfg(not(feature = "chumsky-10"))]
use crate::lexer::{lex_source, lexer, literal, quoted_string};

#[cfg(feature = "chumsky-10")]
use crate::lexer::chumsky_0_10::{lex_source, lexer, literal, quoted_string};

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn line_wrap() {
    assert_debug_snapshot!(Tokens(lexer().parse(r"5 +
    \ 3 "
        ).unwrap()), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            3..9: LineWrap([]),
            10..11: Literal(Integer(3)),
        ],
    )
    ");

    // Comments are included; no newline after the comments
    assert_debug_snapshot!(Tokens(lexer().parse(r"5 +
# comment
   # comment with whitespace
  \ 3 "
        ).unwrap()), @r#"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            3..46: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
            47..48: Literal(Integer(3)),
        ],
    )
    "#);

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

#[cfg_attr(feature = "chumsky-10", ignore)]
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

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn debug_display() {
    assert_debug_snapshot!(Tokens(lexer().parse("5 + 3").unwrap()), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            4..5: Literal(Integer(3)),
        ],
    )
    ");
}

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn comment() {
    assert_debug_snapshot!(Tokens(lexer().parse("# comment\n# second line").unwrap()), @r#"
    Tokens(
        [
            0..9: Comment(" comment"),
            9..10: NewLine,
            10..23: Comment(" second line"),
        ],
    )
    "#);

    assert_snapshot!(TokenKind::Comment(" This is a single-line comment".to_string()), @"# This is a single-line comment");
}

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn doc_comment() {
    assert_debug_snapshot!(Tokens(lexer().parse("#! docs").unwrap()), @r#"
    Tokens(
        [
            0..7: DocComment(" docs"),
        ],
    )
    "#);
}

#[cfg_attr(feature = "chumsky-10", ignore)]
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
    assert_snapshot!(quoted_string(false).parse(r#""""\"hello\""""#).unwrap(), @r#"\"hello\"#);

    assert_snapshot!(quoted_string(true).parse(r#""""\"hello\"""""#).unwrap(), @r#""hello""#);

    // Escape each inner quote depending on the outer quote
    assert_snapshot!(quoted_string(true).parse(r#""\"hello\"""#).unwrap(), @r#""hello""#);
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

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn range() {
    assert_debug_snapshot!(Tokens(lexer().parse("1..2").unwrap()), @r"
    Tokens(
        [
            0..1: Literal(Integer(1)),
            1..3: Range { bind_left: true, bind_right: true },
            3..4: Literal(Integer(2)),
        ],
    )
    ");

    assert_debug_snapshot!(Tokens(lexer().parse("..2").unwrap()), @r"
    Tokens(
        [
            0..2: Range { bind_left: true, bind_right: true },
            2..3: Literal(Integer(2)),
        ],
    )
    ");

    assert_debug_snapshot!(Tokens(lexer().parse("1..").unwrap()), @r"
    Tokens(
        [
            0..1: Literal(Integer(1)),
            1..3: Range { bind_left: true, bind_right: true },
        ],
    )
    ");

    assert_debug_snapshot!(Tokens(lexer().parse("in ..5").unwrap()), @r#"
    Tokens(
        [
            0..2: Ident("in"),
            2..5: Range { bind_left: false, bind_right: true },
            5..6: Literal(Integer(5)),
        ],
    )
    "#);
}

#[cfg_attr(feature = "chumsky-10", ignore)]
#[test]
fn test_lex_source() {
    use insta::assert_debug_snapshot;

    assert_debug_snapshot!(lex_source("5 + 3"), @r"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                4..5: Literal(Integer(3)),
            ],
        ),
    )
    ");

    // Something that will generate an error
    assert_debug_snapshot!(lex_source("^"), @r#"
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
    "#);
}

// New test for chumsky 0.10 implementation
#[cfg(feature = "chumsky-10")]
#[test]
fn test_chumsky_10_lexer() {
    use insta::assert_debug_snapshot;

    // Test basic lexing with the chumsky 0.10 implementation
    assert_debug_snapshot!(lex_source("5 + 3"), @r"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..1: Literal(Integer(5)),
                2..3: Control('+'),
                4..5: Literal(Integer(3)),
            ],
        ),
    )
    ");

    // Test error handling with the chumsky 0.10 implementation
    assert_debug_snapshot!(lex_source("^"), @r#"
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
    "#);
}

// Comprehensive test for Phase III implementation
#[cfg(feature = "chumsky-10")]
#[test]
fn test_chumsky_10_phase3() {
    use insta::assert_debug_snapshot;

    // Test a more complex query with various token types
    let query = r#"
    let x = 5
    from employees
    filter department == "Sales" && salary > 50000
    select {
        name,
        salary,
        # This is a comment
        bonus: salary * 0.1
    }
    "#;

    // Inline snapshot for complex query
    assert_debug_snapshot!(lex_source(query), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..1: NewLine,
                5..8: Keyword("let"),
                9..10: Ident("x"),
                11..12: Control('='),
                13..14: Literal(Integer(5)),
                14..15: NewLine,
                19..23: Ident("from"),
                24..33: Ident("employees"),
                33..34: NewLine,
                38..44: Ident("filter"),
                45..55: Ident("department"),
                56..58: Eq,
                59..66: Literal(String("Sales")),
                67..69: And,
                70..76: Ident("salary"),
                77..78: Control('>'),
                79..84: Literal(Integer(50000)),
                84..85: NewLine,
                89..95: Ident("select"),
                96..97: Control('{'),
                97..98: NewLine,
                106..110: Ident("name"),
                110..111: Control(','),
                111..112: NewLine,
                120..126: Ident("salary"),
                126..127: Control(','),
                127..128: NewLine,
                136..155: Comment(" This is a comment"),
                155..156: NewLine,
                164..169: Ident("bonus"),
                169..170: Control(':'),
                171..177: Ident("salary"),
                178..179: Control('*'),
                180..183: Literal(Float(0.1)),
                183..184: NewLine,
                188..189: Control('}'),
                189..190: NewLine,
            ],
        ),
    )
    "###);
    
    // Test keywords
    assert_debug_snapshot!(lex_source("let into case prql"), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..3: Keyword("let"),
                4..8: Keyword("into"),
                9..13: Keyword("case"),
                14..18: Keyword("prql"),
            ],
        ),
    )
    "###);
    
    // Test operators
    assert_debug_snapshot!(lex_source("-> => == != >="), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..2: ArrowThin,
                3..5: ArrowFat,
                6..8: Eq,
                9..11: Ne,
                12..14: Gte,
            ],
        ),
    )
    "###);
    
    // Test comments
    assert_debug_snapshot!(lex_source("# This is a comment\n#! This is a doc comment"), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..19: Comment(" This is a comment"),
                19..20: NewLine,
                20..44: DocComment(" This is a doc comment"),
            ],
        ),
    )
    "###);
    
    // Test literal and identifier
    assert_debug_snapshot!(lex_source("123 abc"), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..3: Literal(Integer(123)),
                4..7: Ident("abc"),
            ],
        ),
    )
    "###);
    
    // Test boolean and null literals
    assert_debug_snapshot!(lex_source("true false null"), @r###"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..4: Literal(Boolean(true)),
                5..10: Literal(Boolean(false)),
                11..15: Literal(Null),
            ],
        ),
    )
    "###);
}
