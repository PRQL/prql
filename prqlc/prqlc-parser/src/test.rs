use chumsky::Parser;
use insta::{assert_debug_snapshot, assert_yaml_snapshot};
use std::fmt::Debug;

use crate::parser::pr::Stmt;
use crate::parser::prepare_stream;
use crate::parser::stmt;
use crate::{error::Error, lexer::lr::TokenKind, parser::perror::PError};

/// Parse source code based on the supplied parser.
///
/// Use this to test any parser!
pub(crate) fn parse_with_parser<O: Debug>(
    source: &str,
    parser: impl Parser<TokenKind, O, Error = PError>,
) -> Result<O, Vec<Error>> {
    let tokens = crate::lexer::lex_source(source)?;
    let stream = prepare_stream(tokens.0, 0);

    // TODO: possibly should check we consume all the input? Either with an
    // end() parser or some other way (but if we add an end parser then this
    // func doesn't work with `source`, which has its own end parser...)
    let (ast, parse_errors) = parser.parse_recovery_verbose(stream);

    if !parse_errors.is_empty() {
        log::info!("ast: {ast:?}");
        return Err(parse_errors.into_iter().map(|e| e.into()).collect());
    }
    Ok(ast.unwrap())
}

/// Parse into statements
pub(crate) fn parse_source(source: &str) -> Result<Vec<Stmt>, Vec<Error>> {
    parse_with_parser(source, stmt::source())
}

#[test]
fn test_error_unicode_string() {
    // Test various unicode strings successfully parse errors. We were
    // getting loops in the lexer before.
    parse_source("s’ ").unwrap_err();
    parse_source("s’").unwrap_err();
    parse_source(" s’").unwrap_err();
    parse_source(" ’ s").unwrap_err();
    parse_source("’s").unwrap_err();
    parse_source("👍 s’").unwrap_err();

    let source = "Mississippi has four S’s and four I’s.";
    assert_debug_snapshot!(parse_source(source).unwrap_err(), @r###"
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
    assert_debug_snapshot!(parse_source("Answer: T-H-A-T!").unwrap_err(), @r###"
    [
        Error {
            kind: Error,
            span: Some(
                0:6-7,
            ),
            reason: Simple(
                "unexpected :",
            ),
            hints: [],
            code: None,
        },
    ]
    "###);
}

#[test]
fn test_pipeline_parse_tree() {
    assert_yaml_snapshot!(parse_source(
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
    parse_source("take 10").unwrap();

    assert_yaml_snapshot!(parse_source(r#"take 10"#).unwrap(), @r###"
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

    assert_yaml_snapshot!(parse_source(r#"take ..10"#).unwrap(), @r###"
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

    assert_yaml_snapshot!(parse_source(r#"take 1..10"#).unwrap(), @r###"
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
fn test_filter() {
    assert_yaml_snapshot!(
            parse_source(r#"filter country == "USA""#).unwrap(), @r###"
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
        parse_source(r#"filter (text.upper country) == "USA""#).unwrap(), @r###"
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
    let aggregate = parse_source(
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
    let aggregate = parse_source(
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
fn test_basic_exprs() {
    // Currently not putting comments in our parse tree, so this is blank.
    assert_yaml_snapshot!(parse_source(
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
      span: "0:0-36"
    "###);
}

#[test]
fn test_function() {
    assert_yaml_snapshot!(parse_source("let plus_one = x ->  x + 1\n").unwrap(), @r###"
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
    assert_yaml_snapshot!(parse_source("let identity = x ->  x\n").unwrap()
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
    assert_yaml_snapshot!(parse_source("let plus_one = x ->  (x + 1)\n").unwrap()
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
    assert_yaml_snapshot!(parse_source("let plus_one = x ->  x + 1\n").unwrap()
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

    assert_yaml_snapshot!(parse_source("let foo = x -> some_func (foo bar + 1) (plax) - baz\n").unwrap()
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

    assert_yaml_snapshot!(parse_source("func return_constant ->  42\n").unwrap(), @r###"
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
      span: "0:0-27"
    "###);

    assert_yaml_snapshot!(parse_source(r#"let count = X -> s"SUM({X})"
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

    assert_yaml_snapshot!(parse_source(
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
      span: "0:0-147"
    "###);

    assert_yaml_snapshot!(parse_source("let add = x to:a ->  x + to\n").unwrap(), @r###"
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
fn test_var_def() {
    assert_yaml_snapshot!(parse_source(
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

    assert_yaml_snapshot!(parse_source(
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

    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-47"
    "###);

    assert_yaml_snapshot!(parse_source(
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
      span: "0:84-102"
    "###);
}

#[test]
fn test_inline_pipeline() {
    assert_yaml_snapshot!(parse_source("let median = x -> (x | percentile 50)\n").unwrap(), @r###"
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
    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-107"
    "###);
}

#[test]
fn test_tab_characters() {
    // #284
    parse_source(
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

    assert_yaml_snapshot!(parse_source(prql).unwrap(), @r###"
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
      span: "0:0-126"
    "###);
}

#[test]
fn test_sort() {
    assert_yaml_snapshot!(parse_source("
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
      span: "0:0-174"
    "###);
}

#[test]
fn test_dates() {
    assert_yaml_snapshot!(parse_source("
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
      span: "0:0-76"
    "###);
}

#[test]
fn test_multiline_string() {
    assert_yaml_snapshot!(parse_source(r##"
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
                  RawString: r-string test
                span: "0:20-36"
                alias: x
          span: "0:9-36"
      span: "0:0-36"
    "### )
}

#[test]
fn test_empty_lines() {
    // The span of the Pipeline shouldn't include the empty lines; the VarDef
    // should have a larger span
    assert_yaml_snapshot!(parse_source(r#"
from artists
derive x = 5

 

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
                    span: "0:1-5"
                  args:
                    - Ident: artists
                      span: "0:6-13"
                span: "0:1-13"
              - FuncCall:
                  name:
                    Ident: derive
                    span: "0:14-20"
                  args:
                    - Literal:
                        Integer: 5
                      span: "0:25-26"
                      alias: x
                span: "0:14-26"
          span: "0:1-26"
      span: "0:0-26"
    "### )
}

#[test]
fn test_coalesce() {
    assert_yaml_snapshot!(parse_source(r###"
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
      span: "0:0-59"
    "### )
}

#[test]
fn test_literal() {
    assert_yaml_snapshot!(parse_source(r###"
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
      span: "0:0-24"
    "###)
}

#[test]
fn test_allowed_idents() {
    assert_yaml_snapshot!(parse_source(r###"
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
                          span: "0:48-59"
                      span: "0:46-59"
                span: "0:32-60"
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
      span: "0:0-172"
    "###)
}

#[test]
fn test_gt_lt_gte_lte() {
    assert_yaml_snapshot!(parse_source(r###"
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
      span: "0:0-139"
    "###)
}

#[test]
fn test_assign() {
    assert_yaml_snapshot!(parse_source(r###"
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
      span: "0:0-38"
    "###);
}

#[test]
fn test_unicode() {
    let source = "from tète";
    assert_yaml_snapshot!(parse_source(source).unwrap(), @r###"
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
    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-42"
    "###);

    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-25"
    "###);

    assert_yaml_snapshot!(parse_source(r#"
        x
        "#).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          Ident: x
          span: "0:9-10"
      span: "0:0-10"
    "###);
}

#[test]
fn test_array() {
    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-24"
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
      span: "0:24-57"
    "###);
}

#[test]
fn test_annotation() {
    assert_yaml_snapshot!(parse_source(r#"
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
      span: "0:0-61"
      annotations:
        - expr:
            Tuple:
              - Literal:
                  Integer: 1
                span: "0:28-29"
                alias: binding_strength
            span: "0:10-30"
    "###);
    parse_source(
        r#"
        @{binding_strength=1}
        let add = a b -> a + b
        "#,
    )
    .unwrap();

    parse_source(
        r#"
        @{binding_strength=1}
        # comment
        let add = a b -> a + b
        "#,
    )
    .unwrap();

    parse_source(
        r#"
        @{binding_strength=1}


        let add = a b -> a + b
        "#,
    )
    .unwrap();

    parse_source(
        r#"
        @{binding_strength=1}@{binding_strength=2}
        let add = a b -> a + b
        "#,
    )
    .unwrap_err();
}

#[test]
fn check_valid_version() {
    let stmt = format!(
        r#"
        prql version:"{}"
        "#,
        env!("CARGO_PKG_VERSION_MAJOR")
    );
    assert!(parse_source(&stmt).is_ok());

    let stmt = format!(
        r#"
            prql version:"{}.{}"
            "#,
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR")
    );
    assert!(parse_source(&stmt).is_ok());

    let stmt = format!(
        r#"
            prql version:"{}.{}.{}"
            "#,
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );
    assert!(parse_source(&stmt).is_ok());
}

#[test]
fn check_invalid_version() {
    let stmt = format!(
        "prql version:{}\n",
        env!("CARGO_PKG_VERSION_MAJOR").parse::<usize>().unwrap() + 1
    );
    assert!(parse_source(&stmt).is_err());
}

#[test]
fn test_target() {
    assert_yaml_snapshot!(parse_source(
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
      span: "0:34-77"
    "###);
}

#[test]
fn test_number() {
    // We don't allow trailing periods
    assert!(parse_source(
        r#"
    from artists
    derive x = 1."#
    )
    .is_err());
}

#[test]
fn doc_comment() {
    use insta::assert_yaml_snapshot;

    assert_yaml_snapshot!(parse_source(r###"
    from artists
    derive x = 5
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
                    span: "0:5-9"
                  args:
                    - Ident: artists
                      span: "0:10-17"
                span: "0:5-17"
              - FuncCall:
                  name:
                    Ident: derive
                    span: "0:22-28"
                  args:
                    - Literal:
                        Integer: 5
                      span: "0:33-34"
                      alias: x
                span: "0:22-34"
          span: "0:5-34"
      span: "0:0-34"
    "###);

    assert_yaml_snapshot!(parse_source(r###"
    from artists

    #! This is a doc comment

    derive x = 5
    "###).unwrap(), @r###"
    ---
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: from
              span: "0:5-9"
            args:
              - Ident: artists
                span: "0:10-17"
          span: "0:5-17"
      span: "0:0-17"
    - VarDef:
        kind: Main
        name: main
        value:
          FuncCall:
            name:
              Ident: derive
              span: "0:53-59"
            args:
              - Literal:
                  Integer: 5
                span: "0:64-65"
                alias: x
          span: "0:53-65"
      span: "0:47-65"
      doc_comment: " This is a doc comment"
    "###);

    assert_yaml_snapshot!(parse_source(r###"
    #! This is a doc comment
    from artists
    derive x = 5
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
                    span: "0:34-38"
                  args:
                    - Ident: artists
                      span: "0:39-46"
                span: "0:34-46"
              - FuncCall:
                  name:
                    Ident: derive
                    span: "0:51-57"
                  args:
                    - Literal:
                        Integer: 5
                      span: "0:62-63"
                      alias: x
                span: "0:51-63"
          span: "0:34-63"
      span: "0:29-63"
      doc_comment: " This is a doc comment"
    "###);

    assert_debug_snapshot!(parse_source(r###"
    from artists #! This is a doc comment
    "###).unwrap_err(), @r###"
    [
        Error {
            kind: Error,
            span: Some(
                0:18-42,
            ),
            reason: Simple(
                "unexpected #! This is a doc comment\n",
            ),
            hints: [],
            code: None,
        },
    ]
    "###);
}
