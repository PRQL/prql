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
       │       ╰──── main expected type `relation`, but found type `transform relation -> relation`
       │
       │ Help: Have you forgotten an argument to function std.group?
       │
       │ Note: Type `relation` expands to `[tuple_of_scalars]`
    ───╯
    "###);

    // This should suggest parentheses (this might not be an easy one to solve)
    assert_display_snapshot!(compile(r###"
    let f = country -> country == "Canada"

    from employees
    filter f location
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:5:14]
       │
     5 │     filter f location
       │              ────┬───
       │                  ╰───── Unknown name
    ───╯
    "###);

    // Really complicated error message for something so fundamental
    assert_display_snapshot!(compile(r###"
    select tracks
    from artists
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:2:5]
       │
     2 │ ╭─▶     select tracks
     3 │ ├─▶     from artists
       │ │
       │ ╰────────────────────── main expected type `relation`, but found type `infer -> infer`
       │
       │     Help: Have you forgotten an argument to function main?
       │
       │     Note: Type `relation` expands to `[tuple_of_scalars]`
    ───╯
    "###);

    // It's better if we can tell them to put in {} braces
    assert_display_snapshot!(compile(r###"
    from artists
    sort -name
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:11]
       │
     3 │     sort -name
       │           ──┬─
       │             ╰─── Unknown name
    ───╯
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
    assert_display_snapshot!(compile(r###"
    from x
    select f"{}"
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:14]
       │
     3 │     select f"{}"
       │              ┬
       │              ╰── unexpected end of input while parsing interpolated string
    ───╯
    "###);
}
