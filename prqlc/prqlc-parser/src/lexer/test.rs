#[cfg(not(feature = "chumsky-10"))]
use chumsky::Parser;

#[cfg(feature = "chumsky-10")]
use chumsky_0_10::Parser;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;

use crate::lexer::lr::{Literal, TokenKind, Tokens};

// Import lex_source from the module level
use crate::lexer::lex_source;

// Import other needed functions from the respective module based on feature flag
#[cfg(not(feature = "chumsky-10"))]
use crate::lexer::chumsky_0_9::{lexer, literal, quoted_string};

#[cfg(feature = "chumsky-10")]
use crate::lexer::chumsky_0_10::{lexer, literal, quoted_string};

#[cfg(feature = "chumsky-10")]
use chumsky_0_10::input::Stream;

#[test]
fn line_wrap() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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

    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        
        // Basic line wrap
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter(r"5 +
    \ 3 ".chars())).output().unwrap().to_vec()), @r"
        Tokens(
            [
                0..0: Literal(Integer(5)),
                0..0: Control('+'),
                0..0: LineWrap([]),
                0..0: Literal(Integer(3)),
            ],
        )
        ");

        // Comments are included; no newline after the comments
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter(r"5 +
# comment
   # comment with whitespace
  \ 3 ".chars())).output().unwrap().to_vec()), @r#"
        Tokens(
            [
                0..0: Literal(Integer(5)),
                0..0: Control('+'),
                0..0: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
                0..0: Literal(Integer(3)),
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
}

#[test]
fn numbers() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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

    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        
        // Binary notation
        assert_eq!(
            literal().parse(Stream::from_iter("0b1111000011110000".chars())).output().unwrap(),
            &Literal::Integer(61680)
        );
        assert_eq!(
            literal().parse(Stream::from_iter("0b_1111000011110000".chars())).output().unwrap(),
            &Literal::Integer(61680)
        );

        // Hexadecimal notation
        assert_eq!(
            literal().parse(Stream::from_iter("0xff".chars())).output().unwrap(),
            &Literal::Integer(255)
        );
        assert_eq!(
            literal().parse(Stream::from_iter("0x_deadbeef".chars())).output().unwrap(),
            &Literal::Integer(3735928559)
        );

        // Octal notation
        assert_eq!(
            literal().parse(Stream::from_iter("0o777".chars())).output().unwrap(),
            &Literal::Integer(511)
        );
    }
}

#[test]
fn debug_display() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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

    #[cfg(feature = "chumsky-10")]
    {
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter("5 + 3".chars())).output().unwrap().to_vec()), @r"
        Tokens(
            [
                0..0: Literal(Integer(5)),
                0..0: Control('+'),
                0..0: Literal(Integer(3)),
            ],
        )
        ");
    }
}

#[test]
fn comment() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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

    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        use crate::lexer::lr::TokenKind;
        
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter("# comment\n# second line".chars())).output().unwrap().to_vec()), @r#"
        Tokens(
            [
                0..0: Comment(" comment"),
                0..0: NewLine,
                0..0: Comment(" second line"),
            ],
        )
        "#);

        assert_snapshot!(TokenKind::Comment(" This is a single-line comment".to_string()), @"# This is a single-line comment");
    }
}

#[test]
fn doc_comment() {
    #[cfg(not(feature = "chumsky-10"))]
    {
        assert_debug_snapshot!(Tokens(lexer().parse("#! docs").unwrap()), @r#"
        Tokens(
            [
                0..7: DocComment(" docs"),
            ],
        )
        "#);
    }
    
    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter("#! docs".chars())).output().unwrap().to_vec()), @r#"
        Tokens(
            [
                0..0: DocComment(" docs"),
            ],
        )
        "#);
    }
}

#[test]
fn quotes() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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
    
    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        
        // Basic string test for chumsky 0.10
        // For now we just test simple quoted strings as we need to implement triple quotes and escaping
        assert_snapshot!(quoted_string(false).parse(Stream::from_iter(r#"'aoeu'"#.chars())).output().unwrap(), @"aoeu");
        
        // Simple empty string test
        assert_snapshot!(quoted_string(true).parse(Stream::from_iter(r#"''"#.chars())).output().unwrap(), @"");
    }
}

#[test]
fn range() {
    #[cfg(not(feature = "chumsky-10"))]
    {
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
    
    #[cfg(feature = "chumsky-10")]
    {
        use chumsky_0_10::input::Stream;
        
        // Basic range test for now
        assert_debug_snapshot!(Tokens(lexer().parse(Stream::from_iter("1..2".chars())).output().unwrap().to_vec()), @r"
        Tokens(
            [
                0..0: Literal(Integer(1)),
                0..0: Range { bind_left: false, bind_right: false },
                0..0: Literal(Integer(2)),
            ],
        )
        ");
    }
}

#[test]
fn test_lex_source() {
    use insta::assert_debug_snapshot;

    #[cfg(not(feature = "chumsky-10"))]
    {
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
    
    #[cfg(feature = "chumsky-10")]
    {
        use crate::lexer::chumsky_0_10::lex_source;
        
        // Basic success test
        assert_debug_snapshot!(lex_source("5 + 3"), @r"
        Ok(
            Tokens(
                [
                    0..0: Start,
                    0..0: Literal(Integer(5)),
                    0..0: Control('+'),
                    0..0: Literal(Integer(3)),
                ],
            ),
        )
        ");

        // Error test with invalid character (this should be improved in the future)
        assert_debug_snapshot!(lex_source("^"), @r#"
        Err(
            [
                Error {
                    kind: Error,
                    span: None,
                    reason: Unexpected {
                        found: "Lexer error",
                    },
                    hints: [],
                    code: None,
                },
            ],
        )
        "#);
    }
}
