//! Record bad error messages here which we should improve.
//!
//! Some of these will be good issues for new contributors, or for more
//! experienced contributors who would like a quick issue to fix:
//! - Find where the error is being raised now, generally just search for a part
//!   of the message.
//! - Add `` macros to the code to see what's going on.
//! - Write a better message / find a better place to raise a message.
//! - Run `cargo insta test --accept`, and move the test out of this file into
//!   `test_error_messages.rs`. If it's only partially solved, add a TODO and
//!   make a call for where it should go.
//!
//! Adding bad error messages here is also a welcome contribution. Probably
//! one-issue-per-error-message is not a good way of managing them — there would
//! be a huge number of issues, and it would be difficult to see what's current.
//! So instead, add the error message as a test here.

use insta::assert_snapshot;

use super::sql::compile;

#[test]
fn test_bad_error_messages() {
    assert_snapshot!(compile(r###"
    from film
    group
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :3:5 ]
       │
     3 │     group
       │     ──┬──
       │       ╰──── main expected type `relation`, but found type `func transform relation -> relation`
       │
       │ Help: Have you forgotten an argument to function std.group?
       │
       │ Note: Type `relation` expands to `[{..}]`
    ───╯
    ");

    // This should suggest parentheses (this might not be an easy one to solve)
    assert_snapshot!(compile(r#"
    let f = country -> country == "Canada"

    from employees
    filter f location
    "#).unwrap_err(), @r"
    Error:
       ╭─[ :5:14 ]
       │
     5 │     filter f location
       │              ────┬───
       │                  ╰───── Unknown name `location`
    ───╯
    ");

    // Really complicated error message for something so fundamental
    assert_snapshot!(compile(r###"
    select tracks
    from artists
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :3:5 ]
       │
     3 │     from artists
       │     ──────┬─────
       │           ╰─────── expected a function, but found `default_db.artists`
    ───╯
    ");

    // It's better if we can tell them to put in {} braces
    assert_snapshot!(compile(r###"
    from artists
    sort -name
    "###).unwrap_err(), @r"
    Error: expected a pipeline that resolves to a table, but found `internal std.sub`
    ↳ Hint: are you missing `from` statement?
    ");
}

#[test]
fn interpolation_end() {
    use insta::assert_debug_snapshot;

    // This test demonstrates error reporting for an unclosed f-string: `f"{}` (no closing quote).
    // The input ends at position 20 after the `}`, so the closing quote is missing at position 20.
    //
    // The lexer correctly reports the error at position 20 (end of input) with `found: ""`.
    // The parser reports the error at position 21 (character position in line 1) with "unexpected".

    let source = r#"from x | select f"{}"#;

    // LEXER output (for comparison with parser output below):
    assert_debug_snapshot!(prqlc_parser::lexer::lex_source(source).unwrap_err(), @r#"
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
    ]
    "#);

    // PARSER output (full compilation error):
    assert_snapshot!(compile(source).unwrap_err(), @r#"
    Error:
       ╭─[ :1:18 ]
       │
     1 │ from x | select f"{}
       │                  ┬
       │                  ╰── unexpected '"'
    ───╯
    "#);
}

#[test]
fn select_with_extra_fstr() {
    // Should complain in the same way as `select lower "mooo"`
    assert_snapshot!(compile(r#"
    from foo
    select lower f"{x}/{y}"
    "#).unwrap_err(), @"Error: Unknown name `x`");
}

// See also test_error_messages::test_type_error_placement
#[test]
fn misplaced_type_error() {
    // This one should point at `foo` in `select (... foo)`
    // (preferably in addition to the error that is currently generated)
    assert_snapshot!(compile(r###"
    let foo = 123
    from t
    select (true && foo)
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :2:15 ]
       │
     2 │     let foo = 123
       │               ─┬─
       │                ╰─── function std.and, param `right` expected type `bool`, but found type `int`
    ───╯
    ");
}

#[test]
fn invalid_lineage_in_transform() {
    assert_snapshot!(compile(r###"
  from tbl
  group id (
    sort -val
  )
  "###).unwrap_err(), @r"
    Error: expected a pipeline that resolves to a table, but found `internal std.sub`
    ↳ Hint: are you missing `from` statement?
    ");
}

#[test]
fn test_hint_missing_args() {
    assert_snapshot!(compile(r###"
    from film
    select {film_id, lag film_id}
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :3:22 ]
       │
     3 │     select {film_id, lag film_id}
       │                      ─────┬─────
       │                           ╰─────── unexpected `(func offset <int> column <array> -> internal std.lag) film_id`
       │
       │ Help: this is probably a 'bad type' error (we are working on that)
    ───╯
    ")
}

#[test]
fn test_relation_literal_contains_literals() {
    assert_snapshot!(compile(r###"
    [{a=(1+1)}]
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :2:9 ]
       │
     2 │     [{a=(1+1)}]
       │         ──┬──
       │           ╰──── relation literal expected literals, but found ``(std.add ...)``
    ───╯
    ")
}

#[test]
fn nested_groups() {
    // Nested `group` gives a very abstract & internally-focused error message
    assert_snapshot!(compile(r###"
    from invoices
    select {inv = this}
    join item = invoice_items (==invoice_id)

    group { inv.billing_city } (

      group { item.name } (
        aggregate {
          ct1 = count inv.name,
        }
      )
    )
    "###).unwrap_err(), @r"
    Error:
        ╭─[ :9:9 ]
        │
      9 │ ╭─▶         aggregate {
        ┆ ┆
     11 │ ├─▶         }
        │ │
        │ ╰─────────────── internal compiler error; tracked at https://github.com/PRQL/prql/issues/3870
    ────╯
    ");
}

#[test]
fn a_arrow_b() {
    // This is fairly low priority, given how idiosyncratic the query is. If
    // we find other cases, we should increase the priority.
    assert_snapshot!(compile(r###"
    x -> y
    "###).unwrap_err(), @"Error: internal compiler error; tracked at https://github.com/PRQL/prql/issues/4280");
}

#[test]
fn just_std() {
    assert_snapshot!(compile(r###"
    std
    "###).unwrap_err(), @r"
    Error:
       ╭─[ :1:1 ]
       │
     1 │ ╭─▶
     2 │ ├─▶     std
       │ │
       │ ╰───────────── internal compiler error; tracked at https://github.com/PRQL/prql/issues/4474
    ───╯
    ");
}

#[test]
fn empty_tuple_from() {
    assert_snapshot!(compile(r###"
    from {}
    "###).unwrap_err(), @"Error: internal compiler error; tracked at https://github.com/PRQL/prql/issues/4317");

    assert_snapshot!(compile(r###"
    from []
    "###).unwrap_err(), @"Error: internal compiler error; tracked at https://github.com/PRQL/prql/issues/4317");

    assert_snapshot!(compile(r###"
    from {}
    select a
    "###).unwrap_err(), @"Error: internal compiler error; tracked at https://github.com/PRQL/prql/issues/4317");
}
