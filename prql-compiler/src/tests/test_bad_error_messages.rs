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
       ╭─[:?:?]
       │
       │
       │
       │ Help: are you missing `from` statement?
    ───╯
    "###);

    // This should suggest parentheses (this might not be an easy one to solve)
    assert_display_snapshot!(compile(r###"
    func f -> country == "Canada"

    from employees
    filter f location
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:5:14]
       │
     5 │     filter f location
       │              ────┬───
       │                  ╰───── Unknown name location
    ───╯
    "###)
}
