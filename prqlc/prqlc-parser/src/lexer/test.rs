use chumsky_0_10::Parser;
use insta::assert_debug_snapshot;
use insta::assert_snapshot;

use crate::lexer::lex_source;
use crate::lexer::lexer::{lexer, literal, quoted_string};
use crate::lexer::lr::{Literal, TokenKind, Tokens};

#[test]
fn line_wrap() {
    fn test_line_wrap_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

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

    // Comments in line wrap test
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
}

#[test]
fn numbers() {
    fn test_number_parsing(input: &str, expected: Literal) {
        assert_eq!(literal().parse(input).output().unwrap(), &expected);
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
    fn test_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    assert_debug_snapshot!(test_tokens("5 + 3"), @r"
    Tokens(
        [
            0..1: Literal(Integer(5)),
            2..3: Control('+'),
            4..5: Literal(Integer(3)),
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
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    assert_debug_snapshot!(test_comment_tokens("# comment\n# second line"), @r#"
    Tokens(
        [
            0..9: Comment(" comment"),
            9..10: NewLine,
            10..23: Comment(" second line"),
        ],
    )
    "#);
}

#[test]
fn doc_comment() {
    // Unified function to test doccomment tokens
    fn test_doc_comment_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    assert_debug_snapshot!(test_doc_comment_tokens("#! docs"), @r#"
    Tokens(
        [
            0..7: DocComment(" docs"),
        ],
    )
    "#);
}

#[test]
fn quotes() {
    fn test_basic_string(input: &str, escaped: bool, expected_str: &str) {
        let parse_result = quoted_string(escaped).parse(input);
        if let Some(result) = parse_result.output() {
            assert_eq!(result, expected_str);
        } else {
            panic!("Failed to parse string: {:?}", input);
        }
    }

    test_basic_string(r#"'aoeu'"#, false, "aoeu");
    test_basic_string(r#"''"#, true, "");
    test_basic_string(r#""hello""#, true, "hello");
    test_basic_string(r#""hello\nworld""#, true, "hello\nworld");

    // Test escaped quotes
    let basic_escaped = r#""hello\\""#; // Test just a backslash escape
    test_basic_string(basic_escaped, true, "hello\\");

    // Triple-quoted string tests
    test_basic_string(r#"'''aoeu'''"#, false, "aoeu");
    test_basic_string(r#""""aoeu""""#, true, "aoeu");

    // Add more tests for our implementation
    test_basic_string(r#""hello world""#, true, "hello world");
}

#[test]
fn interpolated_strings() {
    // Helper function to test interpolated string tokens
    fn test_interpolation_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    // Test s-string and f-string with regular quotes
    assert_debug_snapshot!(test_interpolation_tokens(r#"s"Hello {name}""#), @r#"
    Tokens(
        [
            0..15: Interpolation('s', "Hello {name}"),
        ],
    )
    "#);

    // Test s-string with triple quotes (important for multi-line SQL in s-strings)
    assert_debug_snapshot!(test_interpolation_tokens(r#"s"""SELECT * FROM table WHERE id = {id}""" "#), @r#"
    Tokens(
        [
            0..42: Interpolation('s', "SELECT * FROM table WHERE id = {id}"),
        ],
    )
    "#);
}

#[test]
fn timestamp_tests() {
    // Helper function to test tokens with timestamps
    fn test_timestamp_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    // Test timestamp with timezone format -08:00 (with colon)
    assert_debug_snapshot!(test_timestamp_tokens("@2020-01-01T13:19:55-08:00"), @r#"
    Tokens(
        [
            0..26: Literal(Timestamp("2020-01-01T13:19:55-0800")),
        ],
    )
    "#);

    // Test timestamp with timezone format Z
    assert_debug_snapshot!(test_timestamp_tokens("@2020-01-02T21:19:55Z"), @r#"
    Tokens(
        [
            0..21: Literal(Timestamp("2020-01-02T21:19:55Z")),
        ],
    )
    "#);
}

#[test]
fn range() {
    fn test_range_tokens(input: &str) -> Tokens {
        Tokens(lexer().parse(input).output().unwrap().to_vec())
    }

    assert_debug_snapshot!(test_range_tokens("1..2"), @r"
    Tokens(
        [
            0..1: Literal(Integer(1)),
            1..3: Range { bind_left: true, bind_right: true },
            3..4: Literal(Integer(2)),
        ],
    )
    ");

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

    let result = test_range_tokens("in ..5");

    // Just verify we have 3 tokens, with the right types and values
    assert_eq!(result.0.len(), 3);

    // Check token types
    assert!(matches!(result.0[0].kind, TokenKind::Ident(ref s) if s == "in"));
    assert!(matches!(result.0[1].kind, TokenKind::Range { .. }));
    assert!(matches!(
        result.0[2].kind,
        TokenKind::Literal(Literal::Integer(5))
    ));
}

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

    let result = lex_source("^");
    assert!(result.is_err());
}

#[test]
fn test_annotation_tokens() {
    use insta::assert_debug_snapshot;

    // Test basic annotation token
    let result = super::debug::lex_debug("@{binding_strength=1}");
    assert_debug_snapshot!(result, @r#"
        Ok(
            Tokens(
                [
                    0..0: Start,
                    0..1: Annotate,
                    1..2: Control('{'),
                    2..18: Ident("binding_strength"),
                    18..19: Control('='),
                    19..20: Literal(Integer(1)),
                    20..21: Control('}'),
                ],
            ),
        )
        "#);

    // Test multi-line annotation
    let result = super::debug::lex_debug(
        r#"
        @{binding_strength=1}
        let add = a b -> a + b
        "#,
    );
    assert_debug_snapshot!(result, @r#"
        Ok(
            Tokens(
                [
                    0..0: Start,
                    0..1: NewLine,
                    9..10: Annotate,
                    10..11: Control('{'),
                    11..27: Ident("binding_strength"),
                    27..28: Control('='),
                    28..29: Literal(Integer(1)),
                    29..30: Control('}'),
                    30..31: NewLine,
                    39..42: Keyword("let"),
                    43..46: Ident("add"),
                    47..48: Control('='),
                    49..50: Ident("a"),
                    51..52: Ident("b"),
                    53..55: ArrowThin,
                    56..57: Ident("a"),
                    58..59: Control('+'),
                    60..61: Ident("b"),
                    61..62: NewLine,
                ],
            ),
        )
        "#);
}

#[test]
fn test_issue_triple_quoted_with_double_quote() {
    use insta::assert_debug_snapshot;

    // The specific test case from ISSUE.md that was failing
    let input = r#""""
''
Canada
"

""""#;
    let result = super::debug::lex_debug(input);
    eprintln!("Result: {:#?}", result);
    assert_debug_snapshot!(result, @r#"
    Ok(
        Tokens(
            [
                0..0: Start,
                0..20: Literal(String("\n''\nCanada\n\"\n\n")),
            ],
        ),
    )
    "#);
}

#[test]
fn test_single_curly_quote() {
    use insta::assert_debug_snapshot;

    // Test what error we get for a single curly quote character
    let input = "’"; // U+2019 RIGHT SINGLE QUOTATION MARK

    eprintln!("\n=== Single Curly Quote Test ===");
    eprintln!("Input: {:?}", input);
    eprintln!("Input bytes: {:?}", input.as_bytes());
    eprintln!(
        "Char 0: {:?} (U+{:04X})",
        input.chars().next().unwrap(),
        input.chars().next().unwrap() as u32
    );

    let result = lex_source(input);
    eprintln!("Result: {:#?}", result);

    assert_debug_snapshot!(result, @r#"
    Err(
        [
            Error {
                kind: Error,
                span: Some(
                    0:0-1,
                ),
                reason: Unexpected {
                    found: "'’'",
                },
                hints: [],
                code: None,
            },
        ],
    )
    "#);
}

#[test]
fn test_mississippi_curly_quotes() {
    use insta::assert_debug_snapshot;

    // Test error reporting for curly quotes (U+2019)
    // This is the Mississippi test case from integration tests
    // NOTE: The quotes in this string are U+2019 RIGHT SINGLE QUOTATION MARK (curly quotes),
    // not U+0027 APOSTROPHE. Make sure your editor preserves them!
    let input = "Mississippi has four S’s and four I’s.";

    eprintln!("\n=== Mississippi Curly Quotes Test ===");
    eprintln!("Input: {:?}", input);
    eprintln!("Input bytes: {:?}", input.as_bytes());
    eprintln!(
        "Char 22: {:?} (U+{:04X})",
        input.chars().nth(22).unwrap(),
        input.chars().nth(22).unwrap() as u32
    );
    eprintln!(
        "Char 35: {:?} (U+{:04X})",
        input.chars().nth(35).unwrap(),
        input.chars().nth(35).unwrap() as u32
    );

    let result1 = lex_source(input);
    eprintln!("{:#?}", result1);

    let (tokens, errors) = super::lexer::lex_source_recovery(input, 1);
    eprintln!("Tokens: {:#?}", tokens);
    eprintln!("Errors: {:#?}", errors);

    assert_debug_snapshot!(result1, @r#"
    Err(
        [
            Error {
                kind: Error,
                span: Some(
                    0:22-23,
                ),
                reason: Unexpected {
                    found: "'’'",
                },
                hints: [],
                code: None,
            },
        ],
    )
    "#);
}

#[test]
fn test_interpolation_empty() {
    use insta::assert_debug_snapshot;

    // Test the f"{}" case that's showing a changed error position
    let input = r#"from x | select f"{}"#;

    eprintln!("\n=== Interpolation Empty Test ===");
    eprintln!("Input: {:?}", input);
    eprintln!("Input bytes: {:?}", input.as_bytes());
    eprintln!(
        "Input length: {} bytes, {} chars",
        input.len(),
        input.chars().count()
    );

    let result = lex_source(input);
    eprintln!("lex_source result: {:#?}", result);

    let (tokens, errors) = super::lexer::lex_source_recovery(input, 1);
    eprintln!("lex_source_recovery tokens: {:#?}", tokens);
    eprintln!("lex_source_recovery errors: {:#?}", errors);

    assert_debug_snapshot!(result, @r#"
    Err(
        [
            Error {
                kind: Error,
                span: Some(
                    0:17-18,
                ),
                reason: Unexpected {
                    found: "'\"'",
                },
                hints: [],
                code: None,
            },
        ],
    )
    "#);
}
