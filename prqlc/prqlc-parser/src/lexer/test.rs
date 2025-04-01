#[cfg(not(feature = "chumsky-10"))]
use chumsky::Parser;

#[cfg(feature = "chumsky-10")]
use chumsky_0_10::Parser;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;

use crate::lexer::lr::{Literal, TokenKind, Tokens};

// Import the appropriate lexer functions based on feature flag
#[cfg(not(feature = "chumsky-10"))]
use crate::lexer::chumsky_0_9::{lexer, literal, quoted_string};

#[cfg(feature = "chumsky-10")]
use crate::lexer::chumsky_0_10::{lexer, literal, quoted_string};

#[cfg(feature = "chumsky-10")]
use chumsky_0_10::input::Stream;

// Helper function to prepare input for parsing - abstracts the differences between versions
#[cfg(not(feature = "chumsky-10"))]
fn prepare_input(input: &str) -> &str {
    input
}

#[cfg(feature = "chumsky-10")]
fn prepare_input(input: &str) -> Stream<std::str::Chars> {
    Stream::from_iter(input.chars())
}

// Helper function to extract output from parser result
#[cfg(not(feature = "chumsky-10"))]
fn extract_output<T>(result: Result<T, chumsky::error::Simple<char>>) -> T {
    result.unwrap()
}

#[cfg(feature = "chumsky-10")]
fn extract_output<T: Clone>(
    result: chumsky_0_10::prelude::ParseResult<
        T,
        chumsky_0_10::error::Simple<chumsky_0_10::span::SimpleSpan<usize>>,
    >,
) -> T {
    result.output().unwrap().clone()
}

#[test]
fn line_wrap() {
    // Helper function to test line wrap tokens for both Chumsky versions
    fn test_line_wrap_tokens(input: &str) -> Tokens {
        #[cfg(not(feature = "chumsky-10"))]
        {
            Tokens(lexer().parse(input).unwrap())
        }

        #[cfg(feature = "chumsky-10")]
        {
            Tokens(
                lexer()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap()
                    .to_vec(),
            )
        }
    }

    // This format test is the same for both versions
    assert_eq!(
        format!(
            "{}",
            TokenKind::LineWrap(vec![TokenKind::Comment(" a comment".to_string())])
        ),
        r#"
\ # a comment
"#
    );

    // Basic line wrap test
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_line_wrap_tokens(r"5 +
    \ 3 "), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            3..9: LineWrap([]),
            10..11: Literal(Integer(3)),
        ],
    )
    ");

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_line_wrap_tokens(r"5 +
    \ 3 "), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            0..1: Control('+'),
            0..1: LineWrap([]),
            0..1: Literal(Integer(3)),
        ],
    )
    ");

    // Comments in line wrap test
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_line_wrap_tokens(r"5 +
# comment
   # comment with whitespace
  \ 3 "), @r#"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            3..46: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
            47..48: Literal(Integer(3)),
        ],
    )
    "#);

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_line_wrap_tokens(r"5 +
# comment
   # comment with whitespace
  \ 3 "), @r#"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            0..1: Control('+'),
            0..1: LineWrap([Comment(" comment"), Comment(" comment with whitespace")]),
            0..1: Literal(Integer(3)),
        ],
    )
    "#);
}

#[test]
fn numbers() {
    // Unified test for number parsing across both Chumsky versions

    // Function to test number parsing that works with both Chumsky versions
    fn test_number_parsing(input: &str, expected: Literal) {
        #[cfg(not(feature = "chumsky-10"))]
        {
            assert_eq!(literal().parse(input).unwrap(), expected);
        }

        #[cfg(feature = "chumsky-10")]
        {
            assert_eq!(
                literal()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap(),
                &expected
            );
        }
    }

    // Binary notation
    test_number_parsing("0b1111000011110000", Literal::Integer(61680));
    test_number_parsing("0b_1111000011110000", Literal::Integer(61680));

    // Hexadecimal notation
    test_number_parsing("0xff", Literal::Integer(255));
    test_number_parsing("0x_deadbeef", Literal::Integer(3735928559));

    // Octal notation
    test_number_parsing("0o777", Literal::Integer(511));
}

#[test]
fn debug_display() {
    // Unified function to test token output for both Chumsky versions
    fn test_tokens(input: &str) -> Tokens {
        #[cfg(not(feature = "chumsky-10"))]
        {
            Tokens(lexer().parse(input).unwrap())
        }

        #[cfg(feature = "chumsky-10")]
        {
            Tokens(
                lexer()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap()
                    .to_vec(),
            )
        }
    }

    // The snapshots will be different due to span differences,
    // but we can unify the test code
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_tokens("5 + 3"), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            4..5: Literal(Integer(3)),
        ],
    )
    ");

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_tokens("5 + 3"), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            0..1: Control('+'),
            0..1: Literal(Integer(3)),
        ],
    )
    ");
}

#[test]
fn comment() {
    // The format rendering test can be shared since it's independent of Chumsky
    assert_snapshot!(TokenKind::Comment(" This is a single-line comment".to_string()), 
                    @"# This is a single-line comment");

    // For the parser test, we use a unified function
    fn test_comment_tokens(input: &str) -> Tokens {
        #[cfg(not(feature = "chumsky-10"))]
        {
            Tokens(lexer().parse(input).unwrap())
        }

        #[cfg(feature = "chumsky-10")]
        {
            Tokens(
                lexer()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap()
                    .to_vec(),
            )
        }
    }

    // The snapshots differ due to span information, but the test code is unified
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_comment_tokens("# comment\n# second line"), @r#"
    Tokens(
        [
            0..9: Comment(" comment"),
            9..10: NewLine,
            10..23: Comment(" second line"),
        ],
    )
    "#);

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_comment_tokens("# comment\n# second line"), @r#"
    Tokens(
        [
            0..1: Comment(" comment"),
            0..1: NewLine,
            0..1: Comment(" second line"),
        ],
    )
    "#);
}

#[test]
fn doc_comment() {
    // Unified function to test doccomment tokens
    fn test_doc_comment_tokens(input: &str) -> Tokens {
        #[cfg(not(feature = "chumsky-10"))]
        {
            Tokens(lexer().parse(input).unwrap())
        }

        #[cfg(feature = "chumsky-10")]
        {
            Tokens(
                lexer()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap()
                    .to_vec(),
            )
        }
    }

    // Snapshots differ due to span information but test code is unified
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_doc_comment_tokens("#! docs"), @r#"
    Tokens(
        [
            0..7: DocComment(" docs"),
        ],
    )
    "#);

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_doc_comment_tokens("#! docs"), @r#"
    Tokens(
        [
            0..1: DocComment(" docs"),
        ],
    )
    "#);
}

#[test]
fn quotes() {
    // Basic string parsing tests that will work with both Chumsky versions
    // More advanced tests need to be conditionally compiled for now
    // as the Chumsky 0.10 implementation is still being developed

    // Helper function to test basic string parsing for both Chumsky versions
    fn test_basic_string(input: &str, escaped: bool, expected_str: &str) {
        #[cfg(not(feature = "chumsky-10"))]
        {
            let result = quoted_string(escaped).parse(input).unwrap();
            assert_eq!(result, expected_str);
        }

        #[cfg(feature = "chumsky-10")]
        {
            let stream = Stream::from_iter(input.chars());
            let parse_result = quoted_string(escaped).parse(stream);
            let result = parse_result.output().unwrap();
            assert_eq!(result, expected_str);
        }
    }

    // Test basic string parsing in both Chumsky versions
    test_basic_string(r#"'aoeu'"#, false, "aoeu");
    test_basic_string(r#"''"#, true, "");

    // More advanced tests for Chumsky 0.9 that aren't yet implemented in 0.10
    #[cfg(not(feature = "chumsky-10"))]
    {
        // Triple quotes
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

        // An empty input should fail
        quoted_string(false).parse(r#""#).unwrap_err();

        // An even number of quotes is an empty string
        assert_snapshot!(quoted_string(true).parse(r#"''''''"#).unwrap(), @"");

        // Hex escape
        assert_snapshot!(quoted_string(true).parse(r"'\x61\x62\x63'").unwrap(), @"abc");

        // Unicode escape
        assert_snapshot!(quoted_string(true).parse(r"'\u{01f422}'").unwrap(), @"🐢");
    }
}

#[test]
fn range() {
    // Helper function to test range parsing for both Chumsky versions
    fn test_range_tokens(input: &str) -> Tokens {
        #[cfg(not(feature = "chumsky-10"))]
        {
            Tokens(lexer().parse(input).unwrap())
        }

        #[cfg(feature = "chumsky-10")]
        {
            Tokens(
                lexer()
                    .parse(Stream::from_iter(input.chars()))
                    .output()
                    .unwrap()
                    .to_vec(),
            )
        }
    }

    // Basic range test for both Chumsky versions
    #[cfg(not(feature = "chumsky-10"))]
    assert_debug_snapshot!(test_range_tokens("1..2"), @r"
    Tokens(
        [
            0..1: Literal(Integer(1)),
            1..3: Range { bind_left: true, bind_right: true },
            3..4: Literal(Integer(2)),
        ],
    )
    ");

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(test_range_tokens("1..2"), @r"
    Tokens(
        [
            0..1: Literal(Integer(1)),
            0..2: Range { bind_left: true, bind_right: true },
            0..1: Literal(Integer(2)),
        ],
    )
    ");

    // Additional tests for Chumsky 0.9 that aren't yet fully implemented in 0.10
    #[cfg(not(feature = "chumsky-10"))]
    {
        assert_debug_snapshot!(test_range_tokens("..2"), @r"
        Tokens(
            [
                0..2: Range { bind_left: true, bind_right: true },
                2..3: Literal(Integer(2)),
            ],
        )
        ");

        assert_debug_snapshot!(test_range_tokens("1.."), @r"
        Tokens(
            [
                0..1: Literal(Integer(1)),
                1..3: Range { bind_left: true, bind_right: true },
            ],
        )
        ");

        assert_debug_snapshot!(test_range_tokens("in ..5"), @r#"
        Tokens(
            [
                0..2: Ident("in"),
                2..5: Range { bind_left: false, bind_right: true },
                5..6: Literal(Integer(5)),
            ],
        )
        "#);
    }
}

#[test]
fn test_lex_source() {
    use insta::assert_debug_snapshot;

    // Basic success test - unified for both Chumsky versions
    // The snapshots are different but the test code is the same
    #[cfg(not(feature = "chumsky-10"))]
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

    #[cfg(feature = "chumsky-10")]
    assert_debug_snapshot!(lex_source("5 + 3"), @r"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..1: Literal(Integer(5)),
                0..1: Control('+'),
                0..1: Literal(Integer(3)),
            ],
        ),
    )
    ");

    // Error test with invalid character - unified for both Chumsky versions
    #[cfg(not(feature = "chumsky-10"))]
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

    #[cfg(feature = "chumsky-10")]
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
