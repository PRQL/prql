use insta::{assert_debug_snapshot, assert_yaml_snapshot};
use itertools::Itertools;

use crate::error::Error;
use crate::lexer;
use crate::parser;
use crate::parser::pr::{Expr, FuncCall, Stmt};

/// Helper that does not track source_ids
fn parse_single(source: &str) -> Result<Vec<Stmt>, Vec<Error>> {
    let tokens = lexer::lex_source(source)?;

    let (ast, parse_errors) = parser::parse_lr_to_pr(source, 0, tokens.0);

    if !parse_errors.is_empty() {
        return Err(parse_errors);
    }
    Ok(ast.unwrap_or_default())
}

fn parse_expr(source: &str) -> Result<Expr, Vec<Error>> {
    let source = format!("let result = ({source}\n)");

    let stmts = parse_single(&source)?;
    let stmt = stmts.into_iter().exactly_one().unwrap();
    Ok(*stmt.kind.into_var_def().unwrap().value.unwrap())
}

#[test]
fn test_error_unicode_string() {
    // Test various unicode strings successfully parse errors. We were
    // getting loops in the lexer before.
    parse_single("s’ ").unwrap_err();
    parse_single("s’").unwrap_err();
    parse_single(" s’").unwrap_err();
    parse_single(" ’ s").unwrap_err();
    parse_single("’s").unwrap_err();
    parse_single("👍 s’").unwrap_err();

    let source = "Mississippi has four S’s and four I’s.";
    assert_debug_snapshot!(parse_single(source).unwrap_err(), @r###"
    [
        Error {
            kind: Error,
            span: Some(
                0:22-23,
            ),
            reason: Unexpected {
                found: "’",
            },
            hints: [],
            code: None,
        },
        Error {
            kind: Error,
            span: Some(
                0:35-36,
            ),
            reason: Unexpected {
                found: "’",
            },
            hints: [],
            code: None,
        },
    ]
    "###);
}

#[test]
fn test_error_unexpected() {
    assert_debug_snapshot!(parse_single("Answer: T-H-A-T!").unwrap_err(), @r###"
    [
        Error {
            kind: Error,
            span: Some(
                0:6-7,
            ),
            reason: Simple(
                "unexpected : while parsing function call",
            ),
            hints: [],
            code: None,
        },
    ]
    "###);
}

#[test]
fn test_pipeline_parse_tree() {
    assert_yaml_snapshot!(parse_single(
        r#"
from employees
filter country == "USA"                      # Each line transforms the previous result.
derive {                                     # This adds columns / variables.
  gross_salary = salary + payroll_tax,
  gross_cost = gross_salary + benefits_cost  # Variables can use other variables.
}
filter gross_cost > 0
group {title, country} (                     # For each group use a nested pipeline
  aggregate {                                # Aggregate each group to a single row
    average salary,
    average gross_salary,
    sum salary,
    sum gross_salary,
    average gross_cost,
    sum_gross_cost = sum gross_cost,
    ct = count salary,
  }
)
sort sum_gross_cost
filter ct > 200
take 20
        "#
    )
    .unwrap());
}

#[test]
fn test_take() {
    parse_single("take 10").unwrap();

    assert_yaml_snapshot!(parse_single(r#"take 10"#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: take
              span: "0:0-4"
            args:
              - Literal:
                  Integer: 10
                span: "0:5-7"
          span: "0:0-7"
      span: "0:0-7"
    "###);

    assert_yaml_snapshot!(parse_single(r#"take ..10"#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: take
              span: "0:0-4"
            args:
              - Range:
                  start: ~
                  end:
                    Literal:
                      Integer: 10
                    span: "0:7-9"
                span: "0:4-9"
          span: "0:0-9"
      span: "0:0-9"
    "###);

    assert_yaml_snapshot!(parse_single(r#"take 1..10"#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: take
              span: "0:0-4"
            args:
              - Range:
                  start:
                    Literal:
                      Integer: 1
                    span: "0:5-6"
                  end:
                    Literal:
                      Integer: 10
                    span: "0:8-10"
                span: "0:5-10"
          span: "0:0-10"
      span: "0:0-10"
    "###);
}

#[test]
fn test_ranges() {
    assert_yaml_snapshot!(parse_expr(r#"3..5"#).unwrap(), @r###"
    ---
    Range:
      start:
        Literal:
          Integer: 3
        span: "0:14-15"
      end:
        Literal:
          Integer: 5
        span: "0:17-18"
    span: "0:13-20"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"-2..-5"#).unwrap(), @r###"
    ---
    Range:
      start:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 2
            span: "0:15-16"
        span: "0:14-16"
      end:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 5
            span: "0:19-20"
        span: "0:18-20"
    span: "0:13-22"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"(-2..(-5 | abs))"#).unwrap(), @r###"
    ---
    Range:
      start:
        Unary:
          op: Neg
          expr:
            Literal:
              Integer: 2
            span: "0:16-17"
        span: "0:15-17"
      end:
        Pipeline:
          exprs:
            - Unary:
                op: Neg
                expr:
                  Literal:
                    Integer: 5
                  span: "0:21-22"
              span: "0:20-22"
            - Ident: abs
              span: "0:25-28"
        span: "0:20-28"
    span: "0:13-32"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"(2 + 5)..'a'"#).unwrap(), @r###"
    ---
    Range:
      start:
        Binary:
          left:
            Literal:
              Integer: 2
            span: "0:15-16"
          op: Add
          right:
            Literal:
              Integer: 5
            span: "0:19-20"
        span: "0:15-20"
      end:
        Literal:
          String: a
        span: "0:23-26"
    span: "0:13-28"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"1.6..rel.col"#).unwrap(), @r###"
    ---
    Range:
      start:
        Literal:
          Float: 1.6
        span: "0:14-17"
      end:
        Indirection:
          base:
            Ident: rel
            span: "0:19-22"
          field:
            Name: col
        span: "0:22-26"
    span: "0:13-28"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"6.."#).unwrap(), @r###"
    ---
    Range:
      start:
        Literal:
          Integer: 6
        span: "0:14-15"
      end: ~
    span: "0:13-19"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"..7"#).unwrap(), @r###"
    ---
    Range:
      start: ~
      end:
        Literal:
          Integer: 7
        span: "0:16-17"
    span: "0:13-19"
    "###);

    assert_yaml_snapshot!(parse_expr(r#".."#).unwrap(), @r###"
    ---
    Range:
      start: ~
      end: ~
    span: "0:13-18"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"@2020-01-01..@2021-01-01"#).unwrap(), @r###"
    ---
    Range:
      start:
        Literal:
          Date: 2020-01-01
        span: "0:14-25"
      end:
        Literal:
          Date: 2021-01-01
        span: "0:27-38"
    span: "0:13-40"
    "###);
}

#[test]
fn test_basic_exprs() {
    assert_yaml_snapshot!(parse_expr(r#"country == "USA""#).unwrap(), @r###"
    ---
    Binary:
      left:
        Ident: country
        span: "0:14-21"
      op: Eq
      right:
        Literal:
          String: USA
        span: "0:25-30"
    span: "0:13-32"
    "###);
    assert_yaml_snapshot!(parse_expr("select {a, b, c}").unwrap(), @r###"
    ---
    FuncCall:
      name:
        Ident: select
        span: "0:14-20"
      args:
        - Tuple:
            - Ident: a
              span: "0:22-23"
            - Ident: b
              span: "0:25-26"
            - Ident: c
              span: "0:28-29"
          span: "0:21-30"
    span: "0:13-32"
    "###);
    assert_yaml_snapshot!(parse_expr(
            "group {title, country} (
                aggregate {sum salary}
            )"
        ).unwrap(), @r###"
    ---
    FuncCall:
      name:
        Ident: group
        span: "0:14-19"
      args:
        - Tuple:
            - Ident: title
              span: "0:21-26"
            - Ident: country
              span: "0:28-35"
          span: "0:20-36"
        - FuncCall:
            name:
              Ident: aggregate
              span: "0:55-64"
            args:
              - Tuple:
                  - FuncCall:
                      name:
                        Ident: sum
                        span: "0:66-69"
                      args:
                        - Ident: salary
                          span: "0:70-76"
                    span: "0:66-76"
                span: "0:65-77"
          span: "0:55-77"
    span: "0:13-93"
    "###);
    assert_yaml_snapshot!(parse_expr(
            r#"    filter country == "USA""#
        ).unwrap(), @r###"
    ---
    FuncCall:
      name:
        Ident: filter
        span: "0:18-24"
      args:
        - Binary:
            left:
              Ident: country
              span: "0:25-32"
            op: Eq
            right:
              Literal:
                String: USA
              span: "0:36-41"
          span: "0:25-41"
    span: "0:13-43"
    "###);
    assert_yaml_snapshot!(parse_expr("{a, b, c,}").unwrap(), @r###"
    ---
    Tuple:
      - Ident: a
        span: "0:15-16"
      - Ident: b
        span: "0:18-19"
      - Ident: c
        span: "0:21-22"
    span: "0:13-26"
    "###);
    assert_yaml_snapshot!(parse_expr(
            r#"{
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost
}"#
        ).unwrap(), @r###"
    ---
    Tuple:
      - Binary:
          left:
            Ident: salary
            span: "0:33-39"
          op: Add
          right:
            Ident: payroll_tax
            span: "0:42-53"
        span: "0:33-53"
        alias: gross_salary
      - Binary:
          left:
            Ident: gross_salary
            span: "0:72-84"
          op: Add
          right:
            Ident: benefits_cost
            span: "0:87-100"
        span: "0:72-100"
        alias: gross_cost
    span: "0:13-104"
    "###);
    // Currently not putting comments in our parse tree, so this is blank.
    assert_yaml_snapshot!(parse_single(
            r#"# this is a comment
        select a"#
        ).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: select
              span: "0:28-34"
            args:
              - Ident: a
                span: "0:35-36"
          span: "0:28-36"
      span: "0:28-36"
      aesthetics_before:
        - Comment: " this is a comment"
    "###);
    assert_yaml_snapshot!(parse_expr(
            "join side:left country (id==employee_id)"
        ).unwrap(), @r###"
    ---
    FuncCall:
      name:
        Ident: join
        span: "0:14-18"
      args:
        - Ident: country
          span: "0:29-36"
        - Binary:
            left:
              Ident: id
              span: "0:38-40"
            op: Eq
            right:
              Ident: employee_id
              span: "0:42-53"
          span: "0:38-53"
      named_args:
        side:
          Ident: left
          span: "0:24-28"
    span: "0:13-56"
    "###);
    assert_yaml_snapshot!(parse_expr("1  + 2").unwrap(), @r###"
    ---
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:14-15"
      op: Add
      right:
        Literal:
          Integer: 2
        span: "0:19-20"
    span: "0:13-22"
    "###)
}

#[test]
fn test_string() {
    let double_quoted_ast = parse_expr(r#"" U S A ""#).unwrap();
    assert_yaml_snapshot!(double_quoted_ast, @r###"
    ---
    Literal:
      String: " U S A "
    span: "0:13-25"
    "###);

    let single_quoted_ast = parse_expr(r#"' U S A '"#).unwrap();
    assert_eq!(single_quoted_ast, double_quoted_ast);

    // Single quotes within double quotes should produce a string containing
    // the single quotes (and vice versa).
    assert_yaml_snapshot!(parse_expr(r#""' U S A '""#).unwrap(), @r###"
    ---
    Literal:
      String: "' U S A '"
    span: "0:13-27"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"'" U S A "'"#).unwrap(), @r###"
    ---
    Literal:
      String: "\" U S A \""
    span: "0:13-27"
    "###);

    parse_expr(r#"" U S A"#).unwrap_err();
    parse_expr(r#"" U S A '"#).unwrap_err();

    assert_yaml_snapshot!(parse_expr(r#"" \nU S A ""#).unwrap(), @r###"
    ---
    Literal:
      String: " \nU S A "
    span: "0:13-27"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"r" \nU S A ""#).unwrap(), @r###"
    ---
    Literal:
      String: " \\nU S A "
    span: "0:13-28"
    "###);

    let multi_double = parse_expr(
        r#""""
''
Canada
"

""""#,
    )
    .unwrap();
    assert_yaml_snapshot!(multi_double, @r###"
    ---
    Literal:
      String: "\n''\nCanada\n\"\n\n"
    span: "0:13-36"
    "###);

    let multi_single = parse_expr(
        r#"'''
Canada
"
"""

'''"#,
    )
    .unwrap();
    assert_yaml_snapshot!(multi_single, @r###"
    ---
    Literal:
      String: "\nCanada\n\"\n\"\"\"\n\n"
    span: "0:13-37"
    "###);

    assert_yaml_snapshot!(
          parse_expr("''").unwrap(),
          @r###"
    ---
    Literal:
      String: ""
    span: "0:13-18"
    "###);
}

#[test]
fn test_s_string() {
    assert_yaml_snapshot!(parse_expr(r#"s"SUM({col})""#).unwrap(), @r###"
    ---
    SString:
      - String: SUM(
      - Expr:
          expr:
            Ident: col
            span: "0:21-24"
          format: ~
      - String: )
    span: "0:13-29"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"s"SUM({rel.`Col name`})""#).unwrap(), @r###"
    ---
    SString:
      - String: SUM(
      - Expr:
          expr:
            Indirection:
              base:
                Ident: rel
                span: "0:21-24"
              field:
                Name: Col name
            span: "0:25-35"
          format: ~
      - String: )
    span: "0:13-40"
    "###)
}

#[test]
fn test_s_string_braces() {
    assert_yaml_snapshot!(parse_expr(r#"s"{{?crystal_var}}""#).unwrap(), @r###"
    ---
    SString:
      - String: "{?crystal_var}"
    span: "0:13-35"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"s"foo{{bar""#).unwrap(), @r###"
    ---
    SString:
      - String: "foo{bar"
    span: "0:13-27"
    "###);
    parse_expr(r#"s"foo{{bar}""#).unwrap_err();
}

#[test]
fn test_tuple() {
    assert_yaml_snapshot!(parse_expr(r#"{1 + 1, 2}"#).unwrap(), @r###"
    ---
    Tuple:
      - Binary:
          left:
            Literal:
              Integer: 1
            span: "0:15-16"
          op: Add
          right:
            Literal:
              Integer: 1
            span: "0:19-20"
        span: "0:15-20"
      - Literal:
          Integer: 2
        span: "0:22-23"
    span: "0:13-26"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"{1 + (f 1), 2}"#).unwrap(), @r###"
    ---
    Tuple:
      - Binary:
          left:
            Literal:
              Integer: 1
            span: "0:15-16"
          op: Add
          right:
            FuncCall:
              name:
                Ident: f
                span: "0:20-21"
              args:
                - Literal:
                    Integer: 1
                  span: "0:22-23"
            span: "0:20-23"
        span: "0:15-24"
      - Literal:
          Integer: 2
        span: "0:26-27"
    span: "0:13-30"
    "###);
    // Line breaks
    assert_yaml_snapshot!(parse_expr(
            r#"{1,

                2}"#
        ).unwrap(), @r###"
    ---
    Tuple:
      - Literal:
          Integer: 1
        span: "0:15-16"
      - Literal:
          Integer: 2
        span: "0:35-36"
    span: "0:13-39"
    "###);
    // Function call in a tuple
    let ab = parse_expr(r#"{a b}"#).unwrap();
    let a_comma_b = parse_expr(r#"{a, b}"#).unwrap();
    assert_yaml_snapshot!(ab, @r###"
    ---
    Tuple:
      - FuncCall:
          name:
            Ident: a
            span: "0:15-16"
          args:
            - Ident: b
              span: "0:17-18"
        span: "0:15-18"
    span: "0:13-21"
    "###);
    assert_yaml_snapshot!(a_comma_b, @r###"
    ---
    Tuple:
      - Ident: a
        span: "0:15-16"
      - Ident: b
        span: "0:18-19"
    span: "0:13-22"
    "###);
    assert_ne!(ab, a_comma_b);

    assert_yaml_snapshot!(parse_expr(r#"{amount, +amount, -amount}"#).unwrap(), @r###"
    ---
    Tuple:
      - Ident: amount
        span: "0:15-21"
      - Unary:
          op: Add
          expr:
            Ident: amount
            span: "0:24-30"
        span: "0:23-30"
      - Unary:
          op: Neg
          expr:
            Ident: amount
            span: "0:33-39"
        span: "0:32-39"
    span: "0:13-42"
    "###);
    // Operators in tuple items
    assert_yaml_snapshot!(parse_expr(r#"{amount, +amount, -amount}"#).unwrap(), @r###"
    ---
    Tuple:
      - Ident: amount
        span: "0:15-21"
      - Unary:
          op: Add
          expr:
            Ident: amount
            span: "0:24-30"
        span: "0:23-30"
      - Unary:
          op: Neg
          expr:
            Ident: amount
            span: "0:33-39"
        span: "0:32-39"
    span: "0:13-42"
    "###);
}

#[test]
fn test_number() {
    assert_yaml_snapshot!(parse_expr(r#"23"#).unwrap(), @r###"
    ---
    Literal:
      Integer: 23
    span: "0:13-18"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"2_3_4.5_6"#).unwrap(), @r###"
    ---
    Literal:
      Float: 234.56
    span: "0:13-25"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"23.6"#).unwrap(), @r###"
    ---
    Literal:
      Float: 23.6
    span: "0:13-20"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"23.0"#).unwrap(), @r###"
    ---
    Literal:
      Float: 23
    span: "0:13-20"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"2 + 2"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Literal:
          Integer: 2
        span: "0:14-15"
      op: Add
      right:
        Literal:
          Integer: 2
        span: "0:18-19"
    span: "0:13-21"
    "###);

    // Underscores at the beginning are parsed as ident
    assert!(parse_expr("_2").unwrap().kind.into_ident().is_ok());
    assert!(parse_expr("_").unwrap().kind.into_ident().is_ok());

    // We don't allow trailing periods
    assert!(parse_expr(r#"add 1. (2, 3)"#).is_err());

    assert!(parse_expr("_2.3").unwrap().kind.is_indirection());

    assert_yaml_snapshot!(parse_expr(r#"2e3"#).unwrap(), @r###"
    ---
    Literal:
      Float: 2000
    span: "0:13-19"
    "###);

    // expr_of_string("2_").unwrap_err(); // TODO
    // expr_of_string("2.3_").unwrap_err(); // TODO
}

#[test]
fn test_filter() {
    assert_yaml_snapshot!(
            parse_single(r#"filter country == "USA""#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: filter
              span: "0:0-6"
            args:
              - Binary:
                  left:
                    Ident: country
                    span: "0:7-14"
                  op: Eq
                  right:
                    Literal:
                      String: USA
                    span: "0:18-23"
                span: "0:7-23"
          span: "0:0-23"
      span: "0:0-23"
    "###);

    assert_yaml_snapshot!(
        parse_single(r#"filter (text.upper country) == "USA""#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: filter
              span: "0:0-6"
            args:
              - Binary:
                  left:
                    FuncCall:
                      name:
                        Indirection:
                          base:
                            Ident: text
                            span: "0:8-12"
                          field:
                            Name: upper
                        span: "0:12-18"
                      args:
                        - Ident: country
                          span: "0:19-26"
                    span: "0:8-26"
                  op: Eq
                  right:
                    Literal:
                      String: USA
                    span: "0:31-36"
                span: "0:7-36"
          span: "0:0-36"
      span: "0:0-36"
    "###
    );
}

#[test]
fn test_aggregate() {
    let aggregate = parse_single(
        r"group {title} (
                aggregate {sum salary, count}
              )",
    )
    .unwrap();
    assert_yaml_snapshot!(
            aggregate, @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: group
              span: "0:0-5"
            args:
              - Tuple:
                  - Ident: title
                    span: "0:7-12"
                span: "0:6-13"
              - FuncCall:
                  name:
                    Ident: aggregate
                    span: "0:32-41"
                  args:
                    - Tuple:
                        - FuncCall:
                            name:
                              Ident: sum
                              span: "0:43-46"
                            args:
                              - Ident: salary
                                span: "0:47-53"
                          span: "0:43-53"
                        - Ident: count
                          span: "0:55-60"
                      span: "0:42-61"
                span: "0:32-61"
          span: "0:0-77"
      span: "0:0-77"
    "###);
    let aggregate = parse_single(
        r"group {title} (
                aggregate {sum salary}
              )",
    )
    .unwrap();
    assert_yaml_snapshot!(
            aggregate, @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: group
              span: "0:0-5"
            args:
              - Tuple:
                  - Ident: title
                    span: "0:7-12"
                span: "0:6-13"
              - FuncCall:
                  name:
                    Ident: aggregate
                    span: "0:32-41"
                  args:
                    - Tuple:
                        - FuncCall:
                            name:
                              Ident: sum
                              span: "0:43-46"
                            args:
                              - Ident: salary
                                span: "0:47-53"
                          span: "0:43-53"
                      span: "0:42-54"
                span: "0:32-54"
          span: "0:0-70"
      span: "0:0-70"
    "###);
}

#[test]
fn test_derive() {
    assert_yaml_snapshot!(
            parse_expr(r#"derive {x = 5, y = (-x)}"#).unwrap()
        , @r###"
    ---
    FuncCall:
      name:
        Ident: derive
        span: "0:14-20"
      args:
        - Tuple:
            - Literal:
                Integer: 5
              span: "0:26-27"
              alias: x
            - Unary:
                op: Neg
                expr:
                  Ident: x
                  span: "0:35-36"
              span: "0:33-37"
              alias: y
          span: "0:21-38"
    span: "0:13-40"
    "###);
}

#[test]
fn test_select() {
    assert_yaml_snapshot!(
            parse_expr(r#"select x"#).unwrap()
        , @r###"
    ---
    FuncCall:
      name:
        Ident: select
        span: "0:14-20"
      args:
        - Ident: x
          span: "0:21-22"
    span: "0:13-24"
    "###);

    assert_yaml_snapshot!(
            parse_expr(r#"select !{x}"#).unwrap()
        , @r###"
    ---
    FuncCall:
      name:
        Ident: select
        span: "0:14-20"
      args:
        - Unary:
            op: Not
            expr:
              Tuple:
                - Ident: x
                  span: "0:23-24"
              span: "0:22-25"
          span: "0:21-25"
    span: "0:13-27"
    "###);

    assert_yaml_snapshot!(
            parse_expr(r#"select {x, y}"#).unwrap()
        , @r###"
    ---
    FuncCall:
      name:
        Ident: select
        span: "0:14-20"
      args:
        - Tuple:
            - Ident: x
              span: "0:22-23"
            - Ident: y
              span: "0:25-26"
          span: "0:21-27"
    span: "0:13-29"
    "###);
}

#[test]
fn test_expr() {
    assert_yaml_snapshot!(
            parse_expr(r#"country == "USA""#).unwrap()
        , @r###"
    ---
    Binary:
      left:
        Ident: country
        span: "0:14-21"
      op: Eq
      right:
        Literal:
          String: USA
        span: "0:25-30"
    span: "0:13-32"
    "###);
    assert_yaml_snapshot!(parse_expr(
                r#"{
  gross_salary = salary + payroll_tax,
  gross_cost   = gross_salary + benefits_cost,
}"#).unwrap(), @r###"
    ---
    Tuple:
      - Binary:
          left:
            Ident: salary
            span: "0:33-39"
          op: Add
          right:
            Ident: payroll_tax
            span: "0:42-53"
        span: "0:33-53"
        alias: gross_salary
      - Binary:
          left:
            Ident: gross_salary
            span: "0:72-84"
          op: Add
          right:
            Ident: benefits_cost
            span: "0:87-100"
        span: "0:72-100"
        alias: gross_cost
    span: "0:13-105"
    "###);
    assert_yaml_snapshot!(
            parse_expr(
                "(salary + payroll_tax) * (1 + tax_rate)"
            ).unwrap(),
            @r###"
    ---
    Binary:
      left:
        Binary:
          left:
            Ident: salary
            span: "0:15-21"
          op: Add
          right:
            Ident: payroll_tax
            span: "0:24-35"
        span: "0:15-35"
      op: Mul
      right:
        Binary:
          left:
            Literal:
              Integer: 1
            span: "0:40-41"
          op: Add
          right:
            Ident: tax_rate
            span: "0:44-52"
        span: "0:40-52"
    span: "0:13-55"
    "###);
}

#[test]
fn test_regex() {
    assert_yaml_snapshot!(
            parse_expr(
                "'oba' ~= 'foobar'"
            ).unwrap(),
            @r###"
    ---
    Binary:
      left:
        Literal:
          String: oba
        span: "0:14-19"
      op: RegexSearch
      right:
        Literal:
          String: foobar
        span: "0:23-31"
    span: "0:13-33"
    "###);
}

#[test]
fn test_function() {
    assert_yaml_snapshot!(parse_single("let plus_one = x ->  x + 1\n").unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: plus_one
        value:
          Func:
            return_ty: ~
            body:
              Binary:
                left:
                  Ident: x
                  span: "0:21-22"
                op: Add
                right:
                  Literal:
                    Integer: 1
                  span: "0:25-26"
              span: "0:21-26"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:15-26"
      span: "0:0-26"
    "###);
    assert_yaml_snapshot!(parse_single("let identity = x ->  x\n").unwrap()
        , @r###"
    ---
    - VarDef:
        kind: Let
        name: identity
        value:
          Func:
            return_ty: ~
            body:
              Ident: x
              span: "0:21-22"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:15-22"
      span: "0:0-22"
    "###);
    assert_yaml_snapshot!(parse_single("let plus_one = x ->  (x + 1)\n").unwrap()
        , @r###"
    ---
    - VarDef:
        kind: Let
        name: plus_one
        value:
          Func:
            return_ty: ~
            body:
              Binary:
                left:
                  Ident: x
                  span: "0:22-23"
                op: Add
                right:
                  Literal:
                    Integer: 1
                  span: "0:26-27"
              span: "0:21-28"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:15-28"
      span: "0:0-28"
    "###);
    assert_yaml_snapshot!(parse_single("let plus_one = x ->  x + 1\n").unwrap()
        , @r###"
    ---
    - VarDef:
        kind: Let
        name: plus_one
        value:
          Func:
            return_ty: ~
            body:
              Binary:
                left:
                  Ident: x
                  span: "0:21-22"
                op: Add
                right:
                  Literal:
                    Integer: 1
                  span: "0:25-26"
              span: "0:21-26"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:15-26"
      span: "0:0-26"
    "###);

    assert_yaml_snapshot!(parse_single("let foo = x -> some_func (foo bar + 1) (plax) - baz\n").unwrap()
        , @r###"
    ---
    - VarDef:
        kind: Let
        name: foo
        value:
          Func:
            return_ty: ~
            body:
              FuncCall:
                name:
                  Ident: some_func
                  span: "0:15-24"
                args:
                  - FuncCall:
                      name:
                        Ident: foo
                        span: "0:26-29"
                      args:
                        - Binary:
                            left:
                              Ident: bar
                              span: "0:30-33"
                            op: Add
                            right:
                              Literal:
                                Integer: 1
                              span: "0:36-37"
                          span: "0:30-37"
                    span: "0:26-37"
                  - Binary:
                      left:
                        Ident: plax
                        span: "0:40-44"
                      op: Sub
                      right:
                        Ident: baz
                        span: "0:48-51"
                    span: "0:39-51"
              span: "0:15-51"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:10-51"
      span: "0:0-51"
    "###);

    assert_yaml_snapshot!(parse_single("func return_constant ->  42\n").unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Func:
            return_ty: ~
            body:
              Literal:
                Integer: 42
              span: "0:25-27"
            params:
              - name: return_constant
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:0-27"
      span: "0:0-28"
    "###);

    assert_yaml_snapshot!(parse_single(r#"let count = X -> s"SUM({X})"
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: count
        value:
          Func:
            return_ty: ~
            body:
              SString:
                - String: SUM(
                - Expr:
                    expr:
                      Ident: X
                      span: "0:24-25"
                    format: ~
                - String: )
              span: "0:17-28"
            params:
              - name: X
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:12-28"
      span: "0:0-28"
    "###);

    assert_yaml_snapshot!(parse_single(
            r#"
            let lag_day = x ->  (
                window x
                by sec_id
                sort date
                lag 1
            )
        "#
        )
        .unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: lag_day
        value:
          Func:
            return_ty: ~
            body:
              Pipeline:
                exprs:
                  - FuncCall:
                      name:
                        Ident: window
                        span: "0:51-57"
                      args:
                        - Ident: x
                          span: "0:58-59"
                    span: "0:51-59"
                  - FuncCall:
                      name:
                        Ident: by
                        span: "0:76-78"
                      args:
                        - Ident: sec_id
                          span: "0:79-85"
                    span: "0:76-85"
                  - FuncCall:
                      name:
                        Ident: sort
                        span: "0:102-106"
                      args:
                        - Ident: date
                          span: "0:107-111"
                    span: "0:102-111"
                  - FuncCall:
                      name:
                        Ident: lag
                        span: "0:128-131"
                      args:
                        - Literal:
                            Integer: 1
                          span: "0:132-133"
                    span: "0:128-133"
              span: "0:33-147"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:27-147"
      span: "0:13-147"
    "###);

    assert_yaml_snapshot!(parse_single("let add = x to:a ->  x + to\n").unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: add
        value:
          Func:
            return_ty: ~
            body:
              Binary:
                left:
                  Ident: x
                  span: "0:21-22"
                op: Add
                right:
                  Ident: to
                  span: "0:25-27"
              span: "0:21-27"
            params:
              - name: x
                default_value: ~
            named_params:
              - name: to
                default_value:
                  Ident: a
                  span: "0:15-16"
            generic_type_params: []
          span: "0:10-27"
      span: "0:0-27"
    "###);
}

#[test]
fn test_func_call() {
    // Function without argument
    let ast = parse_expr(r#"count"#).unwrap();
    let ident = ast.kind.into_ident().unwrap();
    assert_yaml_snapshot!(
            ident, @r###"
    ---
    count
    "###);

    let ast = parse_expr(r#"s 'foo'"#).unwrap();
    assert_yaml_snapshot!(
            ast, @r###"
    ---
    FuncCall:
      name:
        Ident: s
        span: "0:14-15"
      args:
        - Literal:
            String: foo
          span: "0:16-21"
    span: "0:13-23"
    "###);

    // A non-friendly option for #154
    let ast = parse_expr(r#"count s'*'"#).unwrap();
    let func_call: FuncCall = ast.kind.into_func_call().unwrap();
    assert_yaml_snapshot!(
            func_call, @r###"
    ---
    name:
      Ident: count
      span: "0:14-19"
    args:
      - SString:
          - String: "*"
        span: "0:20-24"
    "###);

    parse_expr("plus_one x:0 x:0 ").unwrap_err();

    let ast = parse_expr(r#"add bar to=3"#).unwrap();
    assert_yaml_snapshot!(
            ast, @r###"
    ---
    FuncCall:
      name:
        Ident: add
        span: "0:14-17"
      args:
        - Ident: bar
          span: "0:18-21"
        - Literal:
            Integer: 3
          span: "0:25-26"
          alias: to
    span: "0:13-28"
    "###);
}

#[test]
fn test_right_assoc() {
    assert_yaml_snapshot!(parse_expr(r#"2 ** 3 ** 4"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Literal:
          Integer: 2
        span: "0:14-15"
      op: Pow
      right:
        Binary:
          left:
            Literal:
              Integer: 3
            span: "0:19-20"
          op: Pow
          right:
            Literal:
              Integer: 4
            span: "0:24-25"
        span: "0:19-25"
    span: "0:13-27"
    "###);
    assert_yaml_snapshot!(parse_expr(r#"1 + 2 ** (3 + 4) ** 4"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:14-15"
      op: Add
      right:
        Binary:
          left:
            Literal:
              Integer: 2
            span: "0:18-19"
          op: Pow
          right:
            Binary:
              left:
                Binary:
                  left:
                    Literal:
                      Integer: 3
                    span: "0:24-25"
                  op: Add
                  right:
                    Literal:
                      Integer: 4
                    span: "0:28-29"
                span: "0:24-29"
              op: Pow
              right:
                Literal:
                  Integer: 4
                span: "0:34-35"
            span: "0:23-35"
        span: "0:18-35"
    span: "0:13-37"
    "###);
}

#[test]
fn test_op_precedence() {
    assert_yaml_snapshot!(parse_expr(r#"1 + 2 - 3 - 4"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Binary:
          left:
            Binary:
              left:
                Literal:
                  Integer: 1
                span: "0:14-15"
              op: Add
              right:
                Literal:
                  Integer: 2
                span: "0:18-19"
            span: "0:14-19"
          op: Sub
          right:
            Literal:
              Integer: 3
            span: "0:22-23"
        span: "0:14-23"
      op: Sub
      right:
        Literal:
          Integer: 4
        span: "0:26-27"
    span: "0:13-29"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"1 / (3 * 4)"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Literal:
          Integer: 1
        span: "0:14-15"
      op: DivFloat
      right:
        Binary:
          left:
            Literal:
              Integer: 3
            span: "0:19-20"
          op: Mul
          right:
            Literal:
              Integer: 4
            span: "0:23-24"
        span: "0:19-24"
    span: "0:13-27"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"1 / 2 - 3 * 4 + 1"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Binary:
          left:
            Binary:
              left:
                Literal:
                  Integer: 1
                span: "0:14-15"
              op: DivFloat
              right:
                Literal:
                  Integer: 2
                span: "0:18-19"
            span: "0:14-19"
          op: Sub
          right:
            Binary:
              left:
                Literal:
                  Integer: 3
                span: "0:22-23"
              op: Mul
              right:
                Literal:
                  Integer: 4
                span: "0:26-27"
            span: "0:22-27"
        span: "0:14-27"
      op: Add
      right:
        Literal:
          Integer: 1
        span: "0:30-31"
    span: "0:13-33"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"a && b || !c && d"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Binary:
          left:
            Ident: a
            span: "0:14-15"
          op: And
          right:
            Ident: b
            span: "0:19-20"
        span: "0:14-20"
      op: Or
      right:
        Binary:
          left:
            Unary:
              op: Not
              expr:
                Ident: c
                span: "0:25-26"
            span: "0:24-26"
          op: And
          right:
            Ident: d
            span: "0:30-31"
        span: "0:24-31"
    span: "0:13-33"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"a && b + c || (d e) && f"#).unwrap(), @r###"
    ---
    Binary:
      left:
        Binary:
          left:
            Ident: a
            span: "0:14-15"
          op: And
          right:
            Binary:
              left:
                Ident: b
                span: "0:19-20"
              op: Add
              right:
                Ident: c
                span: "0:23-24"
            span: "0:19-24"
        span: "0:14-24"
      op: Or
      right:
        Binary:
          left:
            FuncCall:
              name:
                Ident: d
                span: "0:29-30"
              args:
                - Ident: e
                  span: "0:31-32"
            span: "0:29-32"
          op: And
          right:
            Ident: f
            span: "0:37-38"
        span: "0:28-38"
    span: "0:13-40"
    "###);
}

#[test]
fn test_var_def() {
    assert_yaml_snapshot!(parse_single(
            "let newest_employees = (from employees)"
        ).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: newest_employees
        value:
          FuncCall:
            name:
              Ident: from
              span: "0:24-28"
            args:
              - Ident: employees
                span: "0:29-38"
          span: "0:23-39"
      span: "0:0-39"
    "###);

    assert_yaml_snapshot!(parse_single(
            r#"
        let newest_employees = (
          from employees
          group country (
            aggregate {
                average_country_salary = average salary
            }
          )
          sort tenure
          take 50
        )"#.trim()).unwrap(),
         @r###"
    ---
    - VarDef:
        kind: Let
        name: newest_employees
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:35-39"
                  args:
                    - Ident: employees
                      span: "0:40-49"
                span: "0:35-49"
              - FuncCall:
                  name:
                    Ident: group
                    span: "0:60-65"
                  args:
                    - Ident: country
                      span: "0:66-73"
                    - FuncCall:
                        name:
                          Ident: aggregate
                          span: "0:88-97"
                        args:
                          - Tuple:
                              - FuncCall:
                                  name:
                                    Ident: average
                                    span: "0:141-148"
                                  args:
                                    - Ident: salary
                                      span: "0:149-155"
                                span: "0:141-155"
                                alias: average_country_salary
                            span: "0:98-169"
                      span: "0:88-169"
                span: "0:60-181"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:192-196"
                  args:
                    - Ident: tenure
                      span: "0:197-203"
                span: "0:192-203"
              - FuncCall:
                  name:
                    Ident: take
                    span: "0:214-218"
                  args:
                    - Literal:
                        Integer: 50
                      span: "0:219-221"
                span: "0:214-221"
          span: "0:23-231"
      span: "0:0-231"
    "###);

    assert_yaml_snapshot!(parse_single(r#"
            let e = s"SELECT * FROM employees"
            "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: e
        value:
          SString:
            - String: SELECT * FROM employees
          span: "0:21-47"
      span: "0:13-47"
    "###);

    assert_yaml_snapshot!(parse_single(
          "let x = (

            from x_table

            select only_in_x = foo

          )

          from x"
        ).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: x
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:23-27"
                  args:
                    - Ident: x_table
                      span: "0:28-35"
                span: "0:23-35"
              - FuncCall:
                  name:
                    Ident: select
                    span: "0:49-55"
                  args:
                    - Ident: foo
                      span: "0:68-71"
                      alias: only_in_x
                span: "0:49-71"
          span: "0:8-84"
      span: "0:0-84"
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: from
              span: "0:96-100"
            args:
              - Ident: x
                span: "0:101-102"
          span: "0:96-102"
      span: "0:96-102"
    "###);
}

#[test]
fn test_inline_pipeline() {
    assert_yaml_snapshot!(parse_expr("(salary | percentile 50)").unwrap(), @r###"
    ---
    Pipeline:
      exprs:
        - Ident: salary
          span: "0:15-21"
        - FuncCall:
            name:
              Ident: percentile
              span: "0:24-34"
            args:
              - Literal:
                  Integer: 50
                span: "0:35-37"
          span: "0:24-37"
    span: "0:13-40"
    "###);
    assert_yaml_snapshot!(parse_single("let median = x -> (x | percentile 50)\n").unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: median
        value:
          Func:
            return_ty: ~
            body:
              Pipeline:
                exprs:
                  - Ident: x
                    span: "0:19-20"
                  - FuncCall:
                      name:
                        Ident: percentile
                        span: "0:23-33"
                      args:
                        - Literal:
                            Integer: 50
                          span: "0:34-36"
                    span: "0:23-36"
              span: "0:18-37"
            params:
              - name: x
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:13-37"
      span: "0:0-37"
    "###);
}

#[test]
fn test_sql_parameters() {
    assert_yaml_snapshot!(parse_single(r#"
        from mytable
        filter {
          first_name == $1,
          last_name == $2.name
        }
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: mytable
                      span: "0:14-21"
                span: "0:9-21"
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:30-36"
                  args:
                    - Tuple:
                        - Binary:
                            left:
                              Ident: first_name
                              span: "0:49-59"
                            op: Eq
                            right:
                              Param: "1"
                              span: "0:63-65"
                          span: "0:49-65"
                        - Binary:
                            left:
                              Ident: last_name
                              span: "0:77-86"
                            op: Eq
                            right:
                              Param: 2.name
                              span: "0:90-97"
                          span: "0:77-97"
                      span: "0:37-107"
                span: "0:30-107"
          span: "0:9-107"
      span: "0:9-108"
    "###);
}

#[test]
fn test_tab_characters() {
    // #284
    parse_single(
        "from c_invoice
join doc:c_doctype (==c_invoice_id)
select [
\tinvoice_no,
\tdocstatus
]",
    )
    .unwrap();
}

#[test]
fn test_backticks() {
    let prql = "
from `a/*.parquet`
aggregate {max c}
join `schema.table` (==id)
join `my-proj.dataset.table`
join `my-proj`.`dataset`.`table`
";

    assert_yaml_snapshot!(parse_single(prql).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:1-5"
                  args:
                    - Ident: a/*.parquet
                      span: "0:6-19"
                span: "0:1-19"
              - FuncCall:
                  name:
                    Ident: aggregate
                    span: "0:20-29"
                  args:
                    - Tuple:
                        - FuncCall:
                            name:
                              Ident: max
                              span: "0:31-34"
                            args:
                              - Ident: c
                                span: "0:35-36"
                          span: "0:31-36"
                      span: "0:30-37"
                span: "0:20-37"
              - FuncCall:
                  name:
                    Ident: join
                    span: "0:38-42"
                  args:
                    - Ident: schema.table
                      span: "0:43-57"
                    - Unary:
                        op: EqSelf
                        expr:
                          Ident: id
                          span: "0:61-63"
                      span: "0:59-63"
                span: "0:38-64"
              - FuncCall:
                  name:
                    Ident: join
                    span: "0:65-69"
                  args:
                    - Ident: my-proj.dataset.table
                      span: "0:70-93"
                span: "0:65-93"
              - FuncCall:
                  name:
                    Ident: join
                    span: "0:94-98"
                  args:
                    - Indirection:
                        base:
                          Indirection:
                            base:
                              Ident: my-proj
                              span: "0:99-108"
                            field:
                              Name: dataset
                          span: "0:108-118"
                        field:
                          Name: table
                      span: "0:118-126"
                span: "0:94-126"
          span: "0:1-126"
      span: "0:1-127"
    "###);
}

#[test]
fn test_sort() {
    assert_yaml_snapshot!(parse_single("
        from invoices
        sort issued_at
        sort (-issued_at)
        sort {issued_at}
        sort {-issued_at}
        sort {issued_at, -amount, +num_of_articles}
        ").unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: invoices
                      span: "0:14-22"
                span: "0:9-22"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:31-35"
                  args:
                    - Ident: issued_at
                      span: "0:36-45"
                span: "0:31-45"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:54-58"
                  args:
                    - Unary:
                        op: Neg
                        expr:
                          Ident: issued_at
                          span: "0:61-70"
                      span: "0:60-70"
                span: "0:54-71"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:80-84"
                  args:
                    - Tuple:
                        - Ident: issued_at
                          span: "0:86-95"
                      span: "0:85-96"
                span: "0:80-96"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:105-109"
                  args:
                    - Tuple:
                        - Unary:
                            op: Neg
                            expr:
                              Ident: issued_at
                              span: "0:112-121"
                          span: "0:111-121"
                      span: "0:110-122"
                span: "0:105-122"
              - FuncCall:
                  name:
                    Ident: sort
                    span: "0:131-135"
                  args:
                    - Tuple:
                        - Ident: issued_at
                          span: "0:137-146"
                        - Unary:
                            op: Neg
                            expr:
                              Ident: amount
                              span: "0:149-155"
                          span: "0:148-155"
                        - Unary:
                            op: Add
                            expr:
                              Ident: num_of_articles
                              span: "0:158-173"
                          span: "0:157-173"
                      span: "0:136-174"
                span: "0:131-174"
          span: "0:9-174"
      span: "0:9-175"
    "###);
}

#[test]
fn test_dates() {
    assert_yaml_snapshot!(parse_single("
        from employees
        derive {age_plus_two_years = (age + 2years)}
        ").unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: employees
                      span: "0:14-23"
                span: "0:9-23"
              - FuncCall:
                  name:
                    Ident: derive
                    span: "0:32-38"
                  args:
                    - Tuple:
                        - Binary:
                            left:
                              Ident: age
                              span: "0:62-65"
                            op: Add
                            right:
                              Literal:
                                ValueAndUnit:
                                  n: 2
                                  unit: years
                              span: "0:68-74"
                          span: "0:61-75"
                          alias: age_plus_two_years
                      span: "0:39-76"
                span: "0:32-76"
          span: "0:9-76"
      span: "0:9-77"
    "###);

    assert_yaml_snapshot!(parse_expr("@2011-02-01").unwrap(), @r###"
    ---
    Literal:
      Date: 2011-02-01
    span: "0:13-27"
    "###);
    assert_yaml_snapshot!(parse_expr("@2011-02-01T10:00").unwrap(), @r###"
    ---
    Literal:
      Timestamp: "2011-02-01T10:00"
    span: "0:13-33"
    "###);
    assert_yaml_snapshot!(parse_expr("@14:00").unwrap(), @r###"
    ---
    Literal:
      Time: "14:00"
    span: "0:13-22"
    "###);
    // assert_yaml_snapshot!(parse_expr("@2011-02-01T10:00<datetime>").unwrap(), @"");

    parse_expr("@2020-01-0").unwrap_err();

    parse_expr("@2020-01-011").unwrap_err();

    parse_expr("@2020-01-01T111").unwrap_err();
}

#[test]
fn test_multiline_string() {
    assert_yaml_snapshot!(parse_single(r##"
        derive x = r"r-string test"
        "##).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: derive
              span: "0:9-15"
            args:
              - Literal:
                  String: r-string test
                span: "0:20-36"
                alias: x
          span: "0:9-36"
      span: "0:9-37"
    "### )
}

#[test]
fn test_coalesce() {
    assert_yaml_snapshot!(parse_single(r###"
        from employees
        derive amount = amount ?? 0
        "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: employees
                      span: "0:14-23"
                span: "0:9-23"
              - FuncCall:
                  name:
                    Ident: derive
                    span: "0:32-38"
                  args:
                    - Binary:
                        left:
                          Ident: amount
                          span: "0:48-54"
                        op: Coalesce
                        right:
                          Literal:
                            Integer: 0
                          span: "0:58-59"
                      span: "0:48-59"
                      alias: amount
                span: "0:32-59"
          span: "0:9-59"
      span: "0:9-60"
    "### )
}

#[test]
fn test_literal() {
    assert_yaml_snapshot!(parse_single(r###"
        derive x = true
        "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: derive
              span: "0:9-15"
            args:
              - Literal:
                  Boolean: true
                span: "0:20-24"
                alias: x
          span: "0:9-24"
      span: "0:9-25"
    "###)
}

#[test]
fn test_allowed_idents() {
    assert_yaml_snapshot!(parse_single(r###"
        from employees
        join _salary (==employee_id) # table with leading underscore
        filter first_name == $1
        select {_employees._underscored_column}
        "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: employees
                      span: "0:14-23"
                span: "0:9-23"
              - FuncCall:
                  name:
                    Ident: join
                    span: "0:32-36"
                  args:
                    - Ident: _salary
                      span: "0:37-44"
                    - Unary:
                        op: EqSelf
                        expr:
                          Ident: employee_id
<<<<<<< HEAD
                      aesthetics_after:
                        - Comment: " table with leading underscore"
||||||| 566e6f14
=======
                          span: "0:48-59"
                      span: "0:46-59"
                span: "0:32-60"
>>>>>>> main
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:101-107"
                  args:
                    - Binary:
                        left:
                          Ident: first_name
                          span: "0:108-118"
                        op: Eq
                        right:
                          Param: "1"
                          span: "0:122-124"
                      span: "0:108-124"
                span: "0:101-124"
              - FuncCall:
                  name:
                    Ident: select
                    span: "0:133-139"
                  args:
                    - Tuple:
                        - Indirection:
                            base:
                              Ident: _employees
                              span: "0:141-151"
                            field:
                              Name: _underscored_column
                          span: "0:141-171"
                      span: "0:140-172"
                span: "0:133-172"
          span: "0:9-172"
      span: "0:9-173"
    "###)
}

#[test]
fn test_gt_lt_gte_lte() {
    assert_yaml_snapshot!(parse_single(r###"
        from people
        filter age >= 100
        filter num_grandchildren <= 10
        filter salary > 0
        filter num_eyes < 2
        "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:9-13"
                  args:
                    - Ident: people
                      span: "0:14-20"
                span: "0:9-20"
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:29-35"
                  args:
                    - Binary:
                        left:
                          Ident: age
                          span: "0:36-39"
                        op: Gte
                        right:
                          Literal:
                            Integer: 100
                          span: "0:43-46"
                      span: "0:36-46"
                span: "0:29-46"
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:55-61"
                  args:
                    - Binary:
                        left:
                          Ident: num_grandchildren
                          span: "0:62-79"
                        op: Lte
                        right:
                          Literal:
                            Integer: 10
                          span: "0:83-85"
                      span: "0:62-85"
                span: "0:55-85"
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:94-100"
                  args:
                    - Binary:
                        left:
                          Ident: salary
                          span: "0:101-107"
                        op: Gt
                        right:
                          Literal:
                            Integer: 0
                          span: "0:110-111"
                      span: "0:101-111"
                span: "0:94-111"
              - FuncCall:
                  name:
                    Ident: filter
                    span: "0:120-126"
                  args:
                    - Binary:
                        left:
                          Ident: num_eyes
                          span: "0:127-135"
                        op: Lt
                        right:
                          Literal:
                            Integer: 2
                          span: "0:138-139"
                      span: "0:127-139"
                span: "0:120-139"
          span: "0:9-139"
      span: "0:9-140"
    "###)
}

#[test]
fn test_assign() {
    assert_yaml_snapshot!(parse_single(r###"
from employees
join s=salaries (==id)
        "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:1-5"
                  args:
                    - Ident: employees
                      span: "0:6-15"
                span: "0:1-15"
              - FuncCall:
                  name:
                    Ident: join
                    span: "0:16-20"
                  args:
                    - Ident: salaries
                      span: "0:23-31"
                      alias: s
                    - Unary:
                        op: EqSelf
                        expr:
                          Ident: id
                          span: "0:35-37"
                      span: "0:33-37"
                span: "0:16-38"
          span: "0:1-38"
      span: "0:1-39"
    "###);
}

#[test]
fn test_ident_with_keywords() {
    assert_yaml_snapshot!(parse_expr(r"select {andrew, orion, lettuce, falsehood, null0}").unwrap(), @r###"
    ---
    FuncCall:
      name:
        Ident: select
        span: "0:14-20"
      args:
        - Tuple:
            - Ident: andrew
              span: "0:22-28"
            - Ident: orion
              span: "0:30-35"
            - Ident: lettuce
              span: "0:37-44"
            - Ident: falsehood
              span: "0:46-55"
            - Ident: null0
              span: "0:57-62"
          span: "0:21-63"
    span: "0:13-65"
    "###);

    assert_yaml_snapshot!(parse_expr(r"{false}").unwrap(), @r###"
    ---
    Tuple:
      - Literal:
          Boolean: false
        span: "0:15-20"
    span: "0:13-23"
    "###);
}

#[test]
fn test_case() {
    assert_yaml_snapshot!(parse_expr(r#"case [
            nickname != null => nickname,
            true => null
        ]"#).unwrap(), @r###"
    ---
    Case:
      - condition:
          Binary:
            left:
              Ident: nickname
              span: "0:33-41"
            op: Ne
            right:
              Literal: "Null"
              span: "0:45-49"
          span: "0:33-49"
        value:
          Ident: nickname
          span: "0:53-61"
      - condition:
          Literal:
            Boolean: true
          span: "0:75-79"
        value:
          Literal: "Null"
          span: "0:83-87"
    span: "0:13-99"
    "###);
}

#[test]
fn test_params() {
    assert_yaml_snapshot!(parse_expr(r#"$2"#).unwrap(), @r###"
    ---
    Param: "2"
    span: "0:13-18"
    "###);

    assert_yaml_snapshot!(parse_expr(r#"$2_any_text"#).unwrap(), @r###"
    ---
    Param: 2_any_text
    span: "0:13-27"
    "###);
}

#[test]
fn test_unicode() {
    let source = "from tète";
    assert_yaml_snapshot!(parse_single(source).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: from
              span: "0:0-4"
            args:
              - Ident: tète
                span: "0:5-9"
          span: "0:0-9"
      span: "0:0-9"
    "###);
}

#[test]
fn test_var_defs() {
    assert_yaml_snapshot!(parse_single(r#"
        let a = (
            x
        )
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: a
        value:
          Ident: x
          span: "0:17-42"
      span: "0:9-42"
    "###);

    assert_yaml_snapshot!(parse_single(r#"
        x
        into a
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Into
        name: a
        value:
          Ident: x
          span: "0:9-10"
      span: "0:9-25"
    "###);

    assert_yaml_snapshot!(parse_single(r#"
        x
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Ident: x
          span: "0:9-10"
      span: "0:9-11"
    "###);
}

#[test]
fn test_array() {
    assert_yaml_snapshot!(parse_single(r#"
        let a = [1, 2,]
        let a = [false, "hello"]
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: a
        value:
          Array:
            - Literal:
                Integer: 1
              span: "0:18-19"
            - Literal:
                Integer: 2
              span: "0:21-22"
          span: "0:17-24"
      span: "0:9-24"
    - VarDef:
        kind: Let
        name: a
        value:
          Array:
            - Literal:
                Boolean: false
              span: "0:42-47"
            - Literal:
                String: hello
              span: "0:49-56"
          span: "0:41-57"
      span: "0:33-57"
    "###);
}

#[test]
fn test_annotation() {
    assert_yaml_snapshot!(parse_single(r#"
        @{binding_strength=1}
        let add = a b -> a + b
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Let
        name: add
        value:
          Func:
            return_ty: ~
            body:
              Binary:
                left:
                  Ident: a
                  span: "0:56-57"
                op: Add
                right:
                  Ident: b
                  span: "0:60-61"
              span: "0:56-61"
            params:
              - name: a
                default_value: ~
              - name: b
                default_value: ~
            named_params: []
            generic_type_params: []
          span: "0:49-61"
      span: "0:9-61"
      annotations:
        - expr:
            Tuple:
              - Literal:
                  Integer: 1
                span: "0:28-29"
                alias: binding_strength
            span: "0:10-30"
    "###);
    parse_single(
        r#"
        @{binding_strength=1} let add = a b -> a + b
        "#,
    )
    .unwrap();

    parse_single(
        r#"
        @{binding_strength=1}
        # comment
        let add = a b -> a + b
        "#,
    )
    .unwrap();

    parse_single(
        r#"
        @{binding_strength=1}


        let add = a b -> a + b
        "#,
    )
    .unwrap();
}

#[test]
fn check_valid_version() {
    let stmt = format!(
        r#"
        prql version:"{}"
        "#,
        env!("CARGO_PKG_VERSION_MAJOR")
    );
    assert!(parse_single(&stmt).is_ok());

    let stmt = format!(
        r#"
            prql version:"{}.{}"
            "#,
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    assert!(parse_single(&stmt).is_ok());

    let stmt = format!(
        r#"
            prql version:"{}.{}.{}"
            "#,
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );
    assert!(parse_single(&stmt).is_ok());
}

#[test]
fn check_invalid_version() {
    let stmt = format!(
        "prql version:{}\n",
        env!("CARGO_PKG_VERSION_MAJOR").parse::<usize>().unwrap() + 1
    );
    assert!(parse_single(&stmt).is_err());
}

#[test]
fn test_target() {
    assert_yaml_snapshot!(parse_single(
            r#"
          prql target:sql.sqlite

          from film
          remove film2
        "#,
        )
        .unwrap(), @r###"
    ---
    - QueryDef:
        version: ~
        other:
          target: sql.sqlite
      span: "0:0-34"
    - VarDef:
        kind: Main
        name: main
        value:
          Pipeline:
            exprs:
              - FuncCall:
                  name:
                    Ident: from
                    span: "0:45-49"
                  args:
                    - Ident: film
                      span: "0:50-54"
                span: "0:45-54"
              - FuncCall:
                  name:
                    Ident: remove
                    span: "0:65-71"
                  args:
                    - Ident: film2
                      span: "0:72-77"
                span: "0:65-77"
          span: "0:45-77"
      span: "0:45-78"
    "###);
}

#[test]
fn test_module() {
    assert_yaml_snapshot!(parse_single(
            r#"
          module hello {
            let world = 1
            let man = module.world
          }
        "#,
        )
        .unwrap(), @r###"
    ---
    - ModuleDef:
        name: hello
        stmts:
          - VarDef:
              kind: Let
              name: world
              value:
                Literal:
                  Integer: 1
                span: "0:50-51"
            span: "0:38-51"
          - VarDef:
              kind: Let
              name: man
              value:
                Indirection:
                  base:
                    Ident: module
                    span: "0:74-80"
                  field:
                    Name: world
                span: "0:74-86"
            span: "0:64-86"
      span: "0:11-98"
    "###);
}

#[test]
fn test_lookup_01() {
    assert_yaml_snapshot!(parse_expr(
    r#"
      {a = {x = 2}}.a.x
    "#,
    ).unwrap(), @r###"
    ---
    Indirection:
      base:
        Indirection:
          base:
            Tuple:
              - Tuple:
                  - Literal:
                      Integer: 2
                    span: "0:31-32"
                    alias: x
                span: "0:26-33"
                alias: a
            span: "0:21-34"
          field:
            Name: a
        span: "0:34-36"
      field:
        Name: x
    span: "0:13-45"
    "###);
}

#[test]
fn test_lookup_02() {
    assert_yaml_snapshot!(parse_expr(
    r#"
      hello.*
    "#,
    ).unwrap(), @r###"
    ---
    Indirection:
      base:
        Ident: hello
        span: "0:21-26"
      field: Star
    span: "0:13-35"
    "###);
}
