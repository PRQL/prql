//! Record bad error messages here which we should improve.
//!
//! Some of these will be good issues for new contributors, or for more
//! experienced contributors who would like a quick issue to fix:
//! - Find where the error is being raised now, generally just search for a part
//!   of the message.
//! - Add `dbg` macros to the code to see what's going on.
//! - Write a better message / find a better place to raise a message.
//! - Run `cargo insta test --accept`, and move the test out of this file into
//!   `test_error_messages.rs`. If it's only partially solved, add a TODO and
//!   make a call for where it should go.
//!
//! Adding bad error messages here is also a welcome contribution. Probably
//! one-issue-per-error-message is not a good way of managing them — there would
//! be a huge number of issues, and it would be difficult to see what's current.
//! So instead, add the error message as a test here.

use super::compile;
use insta::assert_display_snapshot;

#[test]
fn test_bad_error_messages() {
    assert_display_snapshot!(compile(r###"
    from film
    group
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:5]
       │
     3 │     group
       │     ──┬──
       │       ╰──── main expected type `relation`, but found type `func transform relation -> relation`
       │
       │ Help: Have you forgotten an argument to function std.group?
       │
       │ Note: Type `relation` expands to `[tuple]`
    ───╯
    "###);

    // This should suggest parentheses (this might not be an easy one to solve)
    assert_display_snapshot!(compile(r#"
    let f = country -> country == "Canada"

    from employees
    filter f location
    "#).unwrap_err(), @r###"
    Error:
       ╭─[:5:14]
       │
     5 │     filter f location
       │              ────┬───
       │                  ╰───── Unknown name `location`
    ───╯
    "###);

    // Really complicated error message for something so fundamental
    assert_display_snapshot!(compile(r###"
    select tracks
    from artists
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:5]
       │
     3 │     from artists
       │     ──────┬─────
       │           ╰─────── expected a function, but found `default_db.artists`
    ───╯
    "###);

    // It's better if we can tell them to put in {} braces
    assert_display_snapshot!(compile(r###"
    from artists
    sort -name
    "###).unwrap_err(), @r###"
    Error: expected a pipeline that resolves to a table, but found `internal std.sub`
    ↳ Hint: are you missing `from` statement?
    "###);
}

#[test]
fn array_instead_of_tuple() {
    // Particularly given this used to be our syntax, this could be clearer
    // (though we do say so in the message, which is friendly!)
    assert_display_snapshot!(compile(r###"
    from e=employees
    select [e.first_name, e.last_name]
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:12]
       │
     3 │     select [e.first_name, e.last_name]
       │            ─────────────┬─────────────
       │                         ╰─────────────── unexpected `[this.e.first_name, this.e.last_name]`
       │
       │ Help: this is probably a 'bad type' error (we are working on that)
    ───╯
    "###);
}

#[test]
fn empty_interpolations() {
    assert_display_snapshot!(compile(r#"
    from x
    select f"{}"
    "#).unwrap_err(), @r###"
    Error:
       ╭─[:3:14]
       │
     3 │     select f"{}"
       │              ┬
       │              ╰── unexpected end of input while parsing interpolated string
    ───╯
    "###);
}

#[test]
fn select_with_extra_fstr() {
    // Should complain in the same way as `select lower "mooo"`
    assert_display_snapshot!(compile(r#"
    from foo
    select lower f"{x}/{y}"
    "#).unwrap_err(), @r###"
    Error:
       ╭─[:3:20]
       │
     3 │     select lower f"{x}/{y}"
       │                    ─┬─
       │                     ╰─── Unknown name `x`
    ───╯
    "###);
}

// See also test_error_messages::test_type_error_placement
#[test]
fn misplaced_type_error() {
    // This one should point at `foo` in `select (... foo)`
    // (preferably in addition to the error that is currently generated)
    assert_display_snapshot!(compile(r###"
    let foo = 123
    from t
    select (true && foo)
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:2:15]
       │
     2 │     let foo = 123
       │               ─┬─
       │                ╰─── function std.and, param `right` expected type `bool`, but found type `int`
       │
       │ Help: Type `bool` expands to `bool`
    ───╯
    "###);
}

#[test]
fn invalid_lineage_in_transform() {
    assert_display_snapshot!(compile(r###"
  from tbl
  group id (
    sort -val
  )
  "###).unwrap_err(), @r###"
  Error: "Invalid lineage in group, verify the contents of the group call"
  "###);
}

#[test]
fn test_hint_missing_args() {
    assert_display_snapshot!(compile(r###"
    from film
    select {film_id, lag film_id}
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:22]
       │
     3 │     select {film_id, lag film_id}
       │                      ─────┬─────
       │                           ╰─────── unexpected `offset column -> internal std.lag`
       │
       │ Help: this is probably a 'bad type' error (we are working on that)
    ───╯
    "###)
}
