use chumsky::Parser;
use insta::assert_yaml_snapshot;

use super::{new_line, pr::Expr};
use super::{perror::PError, prepare_stream};
use crate::span::Span;
use crate::test::parse_with_parser;
use crate::{error::Error, lexer::lex_source};
use crate::{lexer::lr::TokenKind, parser::pr::FuncCall};

fn parse_expr(source: &str) -> Result<Expr, Vec<Error>> {
    parse_with_parser(
        source,
        new_line().repeated().ignore_then(super::expr::expr_call()),
    )
}

/// Remove leading newlines & the start token, for tests
pub(crate) fn trim_start() -> impl Parser<TokenKind, (), Error = PError> {
    new_line().repeated().ignored()
}

#[test]
fn test_prepare_stream() {
    use insta::assert_yaml_snapshot;

    let input = "from artists | filter name == 'John'";
    let tokens = lex_source(input).unwrap();

    let mut stream = prepare_stream(tokens.0, 0);
    assert_yaml_snapshot!(stream.fetch_tokens().collect::<Vec<(TokenKind, Span)>>(), @r#"
    - - Start
      - "0:0-0"
    - - Ident: from
      - "0:0-4"
    - - Ident: artists
      - "0:5-12"
    - - Control: "|"
      - "0:13-14"
    - - Ident: filter
      - "0:15-21"
    - - Ident: name
      - "0:22-26"
    - - Eq
      - "0:27-29"
    - - Literal:
          String: John
      - "0:30-36"
    "#);
}

#[test]
fn test_ranges() {
    assert_yaml_snapshot!(parse_expr(r#"3..5"#).unwrap(), @r#"
    Range:
      start:
        Literal:
          Integer: 3
        span: "0:0-1"
      end:
        Literal:
          Integer: 5
        span: "0:3-4"
    span: "0:0-4"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"-2..-5"#).unwrap(), @r#"
    Range:
      start:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 2
            span: "0:1-2"
        span: "0:0-2"
      end:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 5
            span: "0:5-6"
        span: "0:4-6"
    span: "0:0-6"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"(-2..(-5 | abs))"#).unwrap(), @r#"
    Range:
      start:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 2
            span: "0:2-3"
        span: "0:1-3"
      end:
        Pipeline:
          exprs:
            - Unary:
                op: Neg
                expr:
                  Literal:
                    Integer: 5
                  span: "0:7-8"
              span: "0:6-8"
            - Ident:
                - abs
              span: "0:11-14"
        span: "0:6-14"
    span: "0:0-16"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"(2 + 5)..'a'"#).unwrap(), @r#"
    Range:
      start:
        Binary:
          left:
            Literal:
              Integer: 2
            span: "0:1-2"
          op: Add
          right:
            Literal:
              Integer: 5
            span: "0:5-6"
        span: "0:1-6"
      end:
        Literal:
          String: a
        span: "0:9-12"
    span: "0:0-12"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"1.6..rel.col"#).unwrap(), @r#"
    Range:
      start:
        Literal:
          Float: 1.6
        span: "0:0-3"
      end:
        Ident:
          - rel
          - col
        span: "0:5-12"
    span: "0:0-12"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"6.."#).unwrap(), @r#"
    Range:
      start:
        Literal:
          Integer: 6
        span: "0:0-1"
      end: ~
    span: "0:0-3"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"..7"#).unwrap(), @r#"
    Range:
      start: ~
      end:
        Literal:
          Integer: 7
        span: "0:2-3"
    span: "0:0-3"
    "#);

    assert_yaml_snapshot!(parse_expr(r#".."#).unwrap(), @r#"
    Range:
      start: ~
      end: ~
    span: "0:0-2"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"@2020-01-01..@2021-01-01"#).unwrap(), @r#"
    Range:
      start:
        Literal:
          Date: 2020-01-01
        span: "0:0-11"
      end:
        Literal:
          Date: 2021-01-01
        span: "0:13-24"
    span: "0:0-24"
    "#);
}

#[test]
fn test_basic_exprs() {
    assert_yaml_snapshot!(parse_expr(r#"country == "USA""#).unwrap(), @r#"
    Binary:
      left:
        Ident:
          - country
        span: "0:0-7"
      op: Eq
      right:
        Literal:
          String: USA
        span: "0:11-16"
    span: "0:0-16"
    "#);
    assert_yaml_snapshot!(parse_expr("select {a, b, c}").unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - select
        span: "0:0-6"
      args:
        - Tuple:
            - Ident:
                - a
              span: "0:8-9"
            - Ident:
                - b
              span: "0:11-12"
            - Ident:
                - c
              span: "0:14-15"
          span: "0:7-16"
    span: "0:0-16"
    "#);
    assert_yaml_snapshot!(parse_expr(
            "group {title, country} (
                aggregate {sum salary}
            )"
        ).unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - group
        span: "0:0-5"
      args:
        - Tuple:
            - Ident:
                - title
              span: "0:7-12"
            - Ident:
                - country
              span: "0:14-21"
          span: "0:6-22"
        - FuncCall:
            name:
              Ident:
                - aggregate
              span: "0:41-50"
            args:
              - Tuple:
                  - FuncCall:
                      name:
                        Ident:
                          - sum
                        span: "0:52-55"
                      args:
                        - Ident:
                            - salary
                          span: "0:56-62"
                    span: "0:52-62"
                span: "0:51-63"
          span: "0:41-63"
    span: "0:0-77"
    "#);
    assert_yaml_snapshot!(parse_expr(
            r#"    filter country == "USA""#
        ).unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - filter
        span: "0:4-10"
      args:
        - Binary:
            left:
              Ident:
                - country
              span: "0:11-18"
            op: Eq
            right:
              Literal:
                String: USA
              span: "0:22-27"
          span: "0:11-27"
    span: "0:4-27"
    "#);
    assert_yaml_snapshot!(parse_expr("{a, b, c,}").unwrap(), @r#"
    Tuple:
      - Ident:
          - a
        span: "0:1-2"
      - Ident:
          - b
        span: "0:4-5"
      - Ident:
          - c
        span: "0:7-8"
    span: "0:0-10"
    "#);
    assert_yaml_snapshot!(parse_expr(
            r#"{
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost
}"#
        ).unwrap(), @r#"
    Tuple:
      - Binary:
          left:
            Ident:
              - salary
            span: "0:19-25"
          op: Add
          right:
            Ident:
              - payroll_tax
            span: "0:28-39"
        span: "0:19-39"
        alias: gross_salary
      - Binary:
          left:
            Ident:
              - gross_salary
            span: "0:58-70"
          op: Add
          right:
            Ident:
              - benefits_cost
            span: "0:73-86"
        span: "0:58-86"
        alias: gross_cost
    span: "0:0-88"
    "#);

    assert_yaml_snapshot!(parse_expr(
            "join side:left country (id==employee_id)"
        ).unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - join
        span: "0:0-4"
      args:
        - Ident:
            - country
          span: "0:15-22"
        - Binary:
            left:
              Ident:
                - id
              span: "0:24-26"
            op: Eq
            right:
              Ident:
                - employee_id
              span: "0:28-39"
          span: "0:24-39"
      named_args:
        side:
          Ident:
            - left
          span: "0:10-14"
    span: "0:0-40"
    "#);
    assert_yaml_snapshot!(parse_expr("1  + 2").unwrap(), @r#"
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:0-1"
      op: Add
      right:
        Literal:
          Integer: 2
        span: "0:5-6"
    span: "0:0-6"
    "#)
}

#[test]
fn test_string() {
    let double_quoted_ast = parse_expr(r#"" U S A ""#).unwrap();
    assert_yaml_snapshot!(double_quoted_ast, @r#"
    Literal:
      String: " U S A "
    span: "0:0-9"
    "#);

    let single_quoted_ast = parse_expr(r#"' U S A '"#).unwrap();
    assert_eq!(single_quoted_ast, double_quoted_ast);

    // Single quotes within double quotes should produce a string containing
    // the single quotes (and vice versa).
    assert_yaml_snapshot!(parse_expr(r#""' U S A '""#).unwrap(), @r#"
    Literal:
      String: "' U S A '"
    span: "0:0-11"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"'" U S A "'"#).unwrap(), @r#"
    Literal:
      String: "\" U S A \""
    span: "0:0-11"
    "#);

    parse_expr(r#"" U S A"#).unwrap_err();
    parse_expr(r#"" U S A '"#).unwrap_err();

    assert_yaml_snapshot!(parse_expr(r#"" \nU S A ""#).unwrap(), @r#"
    Literal:
      String: " \\nU S A "
    span: "0:0-11"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"r" \nU S A ""#).unwrap(), @r#"
    Literal:
      RawString: " \\nU S A "
    span: "0:0-12"
    "#);

    let multi_double = parse_expr(
        r#""""
''
Canada
"

""""#,
    )
    .unwrap();
    assert_yaml_snapshot!(multi_double, @r#"
    Literal:
      String: "\n''\nCanada\n\"\n\n"
    span: "0:0-20"
    "#);

    let multi_single = parse_expr(
        r#"'''
Canada
"
"""

'''"#,
    )
    .unwrap();
    assert_yaml_snapshot!(multi_single, @r#"
    Literal:
      String: "\nCanada\n\"\n\"\"\"\n\n"
    span: "0:0-21"
    "#);

    assert_yaml_snapshot!(
          parse_expr("''").unwrap(),
          @r#"
    Literal:
      String: ""
    span: "0:0-2"
    "#);
}

#[test]
fn test_s_string() {
    assert_yaml_snapshot!(parse_expr(r#"s"SUM({col})""#).unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - s
        span: "0:0-1"
      args:
        - Literal:
            String: "SUM({col})"
          span: "0:1-13"
    span: "0:0-13"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"s"SUM({rel.`Col name`})""#).unwrap(), @r#"
    SString:
      - String: SUM(
      - Expr:
          expr:
            Ident:
              - rel
              - Col name
            span: "0:7-21"
          format: ~
      - String: )
    span: "0:0-24"
    "#)
}

#[test]
fn test_s_string_braces() {
    assert_yaml_snapshot!(parse_expr(r#"s"{{?crystal_var}}""#).unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - s
        span: "0:0-1"
      args:
        - Literal:
            String: "{{?crystal_var}}"
          span: "0:1-19"
    span: "0:0-19"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"s"foo{{bar""#).unwrap(), @r#"
    SString:
      - String: "foo{bar"
    span: "0:0-11"
    "#);
    parse_expr(r#"s"foo{{bar}""#).unwrap_err();
}

#[test]
fn test_tuple() {
    assert_yaml_snapshot!(parse_expr(r#"{1 + 1, 2}"#).unwrap(), @r#"
    Tuple:
      - Binary:
          left:
            Literal:
              Integer: 1
            span: "0:1-2"
          op: Add
          right:
            Literal:
              Integer: 1
            span: "0:5-6"
        span: "0:1-6"
      - Literal:
          Integer: 2
        span: "0:8-9"
    span: "0:0-10"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"{1 + (f 1), 2}"#).unwrap(), @r#"
    Tuple:
      - Binary:
          left:
            Literal:
              Integer: 1
            span: "0:1-2"
          op: Add
          right:
            FuncCall:
              name:
                Ident:
                  - f
                span: "0:6-7"
              args:
                - Literal:
                    Integer: 1
                  span: "0:8-9"
            span: "0:6-9"
        span: "0:1-10"
      - Literal:
          Integer: 2
        span: "0:12-13"
    span: "0:0-14"
    "#);
    // Line breaks
    assert_yaml_snapshot!(parse_expr(
            r#"{1,

                2}"#
        ).unwrap(), @r#"
    Tuple:
      - Literal:
          Integer: 1
        span: "0:1-2"
      - Literal:
          Integer: 2
        span: "0:21-22"
    span: "0:0-23"
    "#);
    // Function call in a tuple
    let ab = parse_expr(r#"{a b}"#).unwrap();
    let a_comma_b = parse_expr(r#"{a, b}"#).unwrap();
    assert_yaml_snapshot!(ab, @r#"
    Tuple:
      - FuncCall:
          name:
            Ident:
              - a
            span: "0:1-2"
          args:
            - Ident:
                - b
              span: "0:3-4"
        span: "0:1-4"
    span: "0:0-5"
    "#);
    assert_yaml_snapshot!(a_comma_b, @r#"
    Tuple:
      - Ident:
          - a
        span: "0:1-2"
      - Ident:
          - b
        span: "0:4-5"
    span: "0:0-6"
    "#);
    assert_ne!(ab, a_comma_b);

    assert_yaml_snapshot!(parse_expr(r#"{amount, +amount, -amount}"#).unwrap(), @r#"
    Tuple:
      - Ident:
          - amount
        span: "0:1-7"
      - Unary:
          op: Add
          expr:
            Ident:
              - amount
            span: "0:10-16"
        span: "0:9-16"
      - Unary:
          op: Neg
          expr:
            Ident:
              - amount
            span: "0:19-25"
        span: "0:18-25"
    span: "0:0-26"
    "#);
    // Operators in tuple items
    assert_yaml_snapshot!(parse_expr(r#"{amount, +amount, -amount}"#).unwrap(), @r#"
    Tuple:
      - Ident:
          - amount
        span: "0:1-7"
      - Unary:
          op: Add
          expr:
            Ident:
              - amount
            span: "0:10-16"
        span: "0:9-16"
      - Unary:
          op: Neg
          expr:
            Ident:
              - amount
            span: "0:19-25"
        span: "0:18-25"
    span: "0:0-26"
    "#);
}

#[test]
fn test_number() {
    assert_yaml_snapshot!(parse_expr(r#"23"#).unwrap(), @r#"
    Literal:
      Integer: 23
    span: "0:0-2"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"2_3_4.5_6"#).unwrap(), @r#"
    Literal:
      Float: 234.56
    span: "0:0-9"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"23.6"#).unwrap(), @r#"
    Literal:
      Float: 23.6
    span: "0:0-4"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"23.0"#).unwrap(), @r#"
    Literal:
      Float: 23
    span: "0:0-4"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"2 + 2"#).unwrap(), @r#"
    Binary:
      left:
        Literal:
          Integer: 2
        span: "0:0-1"
      op: Add
      right:
        Literal:
          Integer: 2
        span: "0:4-5"
    span: "0:0-5"
    "#);

    // Underscores at the beginning are parsed as ident
    assert!(parse_expr("_2").unwrap().kind.into_ident().is_ok());
    assert!(parse_expr("_").unwrap().kind.into_ident().is_ok());

    assert!(parse_expr("_2._3").unwrap().kind.is_ident());

    assert_yaml_snapshot!(parse_expr(r#"2e3"#).unwrap(), @r#"
    FuncCall:
      name:
        Literal:
          Integer: 2
        span: "0:0-1"
      args:
        - Ident:
            - e3
          span: "0:1-3"
    span: "0:0-3"
    "#);

    // expr_of_string("2_").unwrap_err(); // TODO
    // expr_of_string("2.3_").unwrap_err(); // TODO
}

#[test]
fn test_derive() {
    assert_yaml_snapshot!(
            parse_expr(r#"derive {x = 5, y = (-x)}"#).unwrap()
        , @r#"
    FuncCall:
      name:
        Ident:
          - derive
        span: "0:0-6"
      args:
        - Tuple:
            - Literal:
                Integer: 5
              span: "0:12-13"
              alias: x
            - Unary:
                op: Neg
                expr:
                  Ident:
                    - x
                  span: "0:21-22"
              span: "0:19-23"
              alias: y
          span: "0:7-24"
    span: "0:0-24"
    "#);
}

#[test]
fn test_select() {
    assert_yaml_snapshot!(
            parse_expr(r#"select x"#).unwrap()
        , @r#"
    FuncCall:
      name:
        Ident:
          - select
        span: "0:0-6"
      args:
        - Ident:
            - x
          span: "0:7-8"
    span: "0:0-8"
    "#);

    assert_yaml_snapshot!(
            parse_expr(r#"select !{x}"#).unwrap()
        , @r#"
    FuncCall:
      name:
        Ident:
          - select
        span: "0:0-6"
      args:
        - Unary:
            op: Not
            expr:
              Tuple:
                - Ident:
                    - x
                  span: "0:9-10"
              span: "0:8-11"
          span: "0:7-11"
    span: "0:0-11"
    "#);

    assert_yaml_snapshot!(
            parse_expr(r#"select {x, y}"#).unwrap()
        , @r#"
    FuncCall:
      name:
        Ident:
          - select
        span: "0:0-6"
      args:
        - Tuple:
            - Ident:
                - x
              span: "0:8-9"
            - Ident:
                - y
              span: "0:11-12"
          span: "0:7-13"
    span: "0:0-13"
    "#);
}

#[test]
fn test_expr() {
    assert_yaml_snapshot!(
            parse_expr(r#"country == "USA""#).unwrap()
        , @r#"
    Binary:
      left:
        Ident:
          - country
        span: "0:0-7"
      op: Eq
      right:
        Literal:
          String: USA
        span: "0:11-16"
    span: "0:0-16"
    "#);
    assert_yaml_snapshot!(parse_expr(
                r#"{
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost,
}"#).unwrap(), @r#"
    Tuple:
      - Binary:
          left:
            Ident:
              - salary
            span: "0:19-25"
          op: Add
          right:
            Ident:
              - payroll_tax
            span: "0:28-39"
        span: "0:19-39"
        alias: gross_salary
      - Binary:
          left:
            Ident:
              - gross_salary
            span: "0:58-70"
          op: Add
          right:
            Ident:
              - benefits_cost
            span: "0:73-86"
        span: "0:58-86"
        alias: gross_cost
    span: "0:0-89"
    "#);
    assert_yaml_snapshot!(
            parse_expr(
                "(salary + payroll_tax) * (1 + tax_rate)"
            ).unwrap(),
            @r#"
    Binary:
      left:
        Binary:
          left:
            Ident:
              - salary
            span: "0:1-7"
          op: Add
          right:
            Ident:
              - payroll_tax
            span: "0:10-21"
        span: "0:1-21"
      op: Mul
      right:
        Binary:
          left:
            Literal:
              Integer: 1
            span: "0:26-27"
          op: Add
          right:
            Ident:
              - tax_rate
            span: "0:30-38"
        span: "0:26-38"
    span: "0:0-39"
    "#);
}

#[test]
fn test_regex() {
    assert_yaml_snapshot!(
            parse_expr(
                "'oba' ~= 'foobar'"
            ).unwrap(),
            @r#"
    Binary:
      left:
        Literal:
          String: oba
        span: "0:0-5"
      op: RegexSearch
      right:
        Literal:
          String: foobar
        span: "0:9-17"
    span: "0:0-17"
    "#);
}

#[test]
fn test_func_call() {
    // Function without argument
    let ast = parse_expr(r#"count"#).unwrap();
    let ident = ast.kind.into_ident().unwrap();
    assert_yaml_snapshot!(
            ident, @"- count");

    let ast = parse_expr(r#"s 'foo'"#).unwrap();
    assert_yaml_snapshot!(
            ast, @r#"
    FuncCall:
      name:
        Ident:
          - s
        span: "0:0-1"
      args:
        - Literal:
            String: foo
          span: "0:2-7"
    span: "0:0-7"
    "#);

    // A non-friendly option for #154
    let ast = parse_expr(r#"count s'*'"#).unwrap();
    let func_call: FuncCall = ast.kind.into_func_call().unwrap();
    assert_yaml_snapshot!(
            func_call, @r#"
    name:
      Ident:
        - count
      span: "0:0-5"
    args:
      - Ident:
          - s
        span: "0:6-7"
      - Literal:
          String: "*"
        span: "0:7-10"
    "#);

    parse_expr("plus_one x:0 x:0 ").unwrap_err();

    let ast = parse_expr(r#"add bar to=3"#).unwrap();
    assert_yaml_snapshot!(
            ast, @r#"
    FuncCall:
      name:
        Ident:
          - add
        span: "0:0-3"
      args:
        - Ident:
            - bar
          span: "0:4-7"
        - Literal:
            Integer: 3
          span: "0:11-12"
          alias: to
    span: "0:0-12"
    "#);
}

#[test]
fn test_right_assoc() {
    assert_yaml_snapshot!(parse_expr(r#"2 ** 3 ** 4"#).unwrap(), @r#"
    Binary:
      left:
        Literal:
          Integer: 2
        span: "0:0-1"
      op: Pow
      right:
        Binary:
          left:
            Literal:
              Integer: 3
            span: "0:5-6"
          op: Pow
          right:
            Literal:
              Integer: 4
            span: "0:10-11"
        span: "0:5-11"
    span: "0:0-11"
    "#);
    assert_yaml_snapshot!(parse_expr(r#"1 + 2 ** (3 + 4) ** 4"#).unwrap(), @r#"
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:0-1"
      op: Add
      right:
        Binary:
          left:
            Literal:
              Integer: 2
            span: "0:4-5"
          op: Pow
          right:
            Binary:
              left:
                Binary:
                  left:
                    Literal:
                      Integer: 3
                    span: "0:10-11"
                  op: Add
                  right:
                    Literal:
                      Integer: 4
                    span: "0:14-15"
                span: "0:10-15"
              op: Pow
              right:
                Literal:
                  Integer: 4
                span: "0:20-21"
            span: "0:9-21"
        span: "0:4-21"
    span: "0:0-21"
    "#);
}

#[test]
fn test_op_precedence() {
    assert_yaml_snapshot!(parse_expr(r#"1 + 2 - 3 - 4"#).unwrap(), @r#"
    Binary:
      left:
        Binary:
          left:
            Binary:
              left:
                Literal:
                  Integer: 1
                span: "0:0-1"
              op: Add
              right:
                Literal:
                  Integer: 2
                span: "0:4-5"
            span: "0:0-5"
          op: Sub
          right:
            Literal:
              Integer: 3
            span: "0:8-9"
        span: "0:0-9"
      op: Sub
      right:
        Literal:
          Integer: 4
        span: "0:12-13"
    span: "0:0-13"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"1 / (3 * 4)"#).unwrap(), @r#"
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:0-1"
      op: DivFloat
      right:
        Binary:
          left:
            Literal:
              Integer: 3
            span: "0:5-6"
          op: Mul
          right:
            Literal:
              Integer: 4
            span: "0:9-10"
        span: "0:5-10"
    span: "0:0-11"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"1 / 2 - 3 * 4 + 1"#).unwrap(), @r#"
    Binary:
      left:
        Binary:
          left:
            Binary:
              left:
                Literal:
                  Integer: 1
                span: "0:0-1"
              op: DivFloat
              right:
                Literal:
                  Integer: 2
                span: "0:4-5"
            span: "0:0-5"
          op: Sub
          right:
            Binary:
              left:
                Literal:
                  Integer: 3
                span: "0:8-9"
              op: Mul
              right:
                Literal:
                  Integer: 4
                span: "0:12-13"
            span: "0:8-13"
        span: "0:0-13"
      op: Add
      right:
        Literal:
          Integer: 1
        span: "0:16-17"
    span: "0:0-17"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"a && b || !c && d"#).unwrap(), @r#"
    Binary:
      left:
        Binary:
          left:
            Ident:
              - a
            span: "0:0-1"
          op: And
          right:
            Ident:
              - b
            span: "0:5-6"
        span: "0:0-6"
      op: Or
      right:
        Binary:
          left:
            Unary:
              op: Not
              expr:
                Ident:
                  - c
                span: "0:11-12"
            span: "0:10-12"
          op: And
          right:
            Ident:
              - d
            span: "0:16-17"
        span: "0:10-17"
    span: "0:0-17"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"a && b + c || (d e) && f"#).unwrap(), @r#"
    Binary:
      left:
        Binary:
          left:
            Ident:
              - a
            span: "0:0-1"
          op: And
          right:
            Binary:
              left:
                Ident:
                  - b
                span: "0:5-6"
              op: Add
              right:
                Ident:
                  - c
                span: "0:9-10"
            span: "0:5-10"
        span: "0:0-10"
      op: Or
      right:
        Binary:
          left:
            FuncCall:
              name:
                Ident:
                  - d
                span: "0:15-16"
              args:
                - Ident:
                    - e
                  span: "0:17-18"
            span: "0:15-18"
          op: And
          right:
            Ident:
              - f
            span: "0:23-24"
        span: "0:14-24"
    span: "0:0-24"
    "#);
}

#[test]
fn test_inline_pipeline() {
    assert_yaml_snapshot!(parse_expr("(salary | percentile 50)").unwrap(), @r#"
    Pipeline:
      exprs:
        - Ident:
            - salary
          span: "0:1-7"
        - FuncCall:
            name:
              Ident:
                - percentile
              span: "0:10-20"
            args:
              - Literal:
                  Integer: 50
                span: "0:21-23"
          span: "0:10-23"
    span: "0:0-24"
    "#);
}

#[test]
fn test_dates() {
    assert_yaml_snapshot!(parse_expr("@2011-02-01").unwrap(), @r#"
    Literal:
      Date: 2011-02-01
    span: "0:0-11"
    "#);
    assert_yaml_snapshot!(parse_expr("@2011-02-01T10:00").unwrap(), @r#"
    Literal:
      Timestamp: "2011-02-01T10:00"
    span: "0:0-17"
    "#);
    assert_yaml_snapshot!(parse_expr("@14:00").unwrap(), @r#"
    Literal:
      Time: "14:00"
    span: "0:0-6"
    "#);
    // assert_yaml_snapshot!(parse_expr("@2011-02-01T10:00<datetime>").unwrap(), @"");

    parse_expr("@2020-01-0").unwrap_err();

    parse_expr("@2020-01-011").unwrap_err();

    parse_expr("@2020-01-01T111").unwrap_err();
}

#[test]
fn test_ident_with_keywords() {
    assert_yaml_snapshot!(parse_expr(r"select {andrew, orion, lettuce, falsehood, null0}").unwrap(), @r#"
    FuncCall:
      name:
        Ident:
          - select
        span: "0:0-6"
      args:
        - Tuple:
            - Ident:
                - andrew
              span: "0:8-14"
            - Ident:
                - orion
              span: "0:16-21"
            - Ident:
                - lettuce
              span: "0:23-30"
            - Ident:
                - falsehood
              span: "0:32-41"
            - Ident:
                - null0
              span: "0:43-48"
          span: "0:7-49"
    span: "0:0-49"
    "#);

    assert_yaml_snapshot!(parse_expr(r"{false}").unwrap(), @r#"
    Tuple:
      - Literal:
          Boolean: false
        span: "0:1-6"
    span: "0:0-7"
    "#);
}

#[test]
fn test_case() {
    assert_yaml_snapshot!(parse_expr(r#"
        case [
            nickname != null => nickname,
            true => null
        ]
        "#).unwrap(), @r#"
    Case:
      - condition:
          Binary:
            left:
              Ident:
                - nickname
              span: "0:28-36"
            op: Ne
            right:
              Literal: "Null"
              span: "0:40-44"
          span: "0:28-44"
        value:
          Ident:
            - nickname
          span: "0:48-56"
      - condition:
          Literal:
            Boolean: true
          span: "0:70-74"
        value:
          Literal: "Null"
          span: "0:78-82"
    span: "0:9-92"
    "#);
}

#[test]
fn test_params() {
    assert_yaml_snapshot!(parse_expr(r#"$2"#).unwrap(), @r#"
    Param: "2"
    span: "0:0-2"
    "#);

    assert_yaml_snapshot!(parse_expr(r#"$2_any_text"#).unwrap(), @r#"
    Param: 2_any_text
    span: "0:0-11"
    "#);
}

#[test]
fn test_lookup_01() {
    assert_yaml_snapshot!(parse_expr(
    r#"{a = {x = 2}}.a.x"#,
    ).unwrap(), @r#"
    Tuple:
      - Tuple:
          - Literal:
              Integer: 2
            span: "0:10-11"
            alias: x
        span: "0:5-12"
        alias: a
    span: "0:0-13"
    "#);
}

#[test]
fn test_lookup_02() {
    assert_yaml_snapshot!(parse_expr(
    r#"hello.*"#,
    ).unwrap(), @r#"
    Ident:
      - hello
      - "*"
    span: "0:0-7"
    "#);
}
