//! Test error messages. As of 2023-03, this can hopefully be expanded significantly.
//! It's also fine to put errors by the things that they're testing.
//! See also [test_bad_error_messages.rs](test_bad_error_messages.rs) for error
//! messages which need to be improved.

use super::compile;
use insta::assert_display_snapshot;

#[test]
fn test_errors() {
    assert_display_snapshot!(compile(r###"
    func addadd a b -> a + b

    from x
    derive y = (addadd 4 5 6)
    "###).unwrap_err(),
        @r###"
    Error:
       ╭─[:5:16]
       │
     5 │     derive y = (addadd 4 5 6)
       │                ───────┬──────
       │                       ╰──────── Too many arguments to function `addadd`
    ───╯
    "###);

    assert_display_snapshot!(compile(r###"
    from a select b
    "###).unwrap_err(),
        @r###"
    Error:
       ╭─[:2:5]
       │
     2 │     from a select b
       │     ────────┬───────
       │             ╰───────── Too many arguments to function `from`
    ───╯
    "###);

    assert_display_snapshot!(compile(r###"
    from x
    select a
    select b
    "###).unwrap_err(),
        @r###"
    Error:
       ╭─[:4:12]
       │
     4 │     select b
       │            ┬
       │            ╰── Unknown name b
    ───╯
    "###);

    assert_display_snapshot!(compile(r###"
    from employees
    take 1.8
    "###).unwrap_err(),
        @r###"
    Error:
       ╭─[:3:10]
       │
     3 │     take 1.8
       │          ─┬─
       │           ╰─── `take` expected int or range, but found 1.8
    ───╯
    "###);

    assert_display_snapshot!(compile("Mississippi has four S’s and four I’s.").unwrap_err(), @r###"
    Error:
       ╭─[:1:23]
       │
     1 │ Mississippi has four S’s and four I’s.
       │                       ┬
       │                       ╰── unexpected ’
    ───╯
    Error:
       ╭─[:1:36]
       │
     1 │ Mississippi has four S’s and four I’s.
       │                                    ┬
       │                                    ╰── unexpected ’
    ───╯
    Error:
       ╭─[:1:38]
       │
     1 │ Mississippi has four S’s and four I’s.
       │                                      ┬
       │                                      ╰── Expected * or an identifier, but didn't find anything before the end.
    ───╯
    "###);

    let err = compile(
        r###"
    let a = (from x)
    "###,
    )
    .unwrap_err();
    assert_eq!(err.inner[0].code.as_ref().unwrap(), "E0001");

    assert_display_snapshot!(compile("Answer: T-H-A-T!").unwrap_err(), @r###"
    Error:
       ╭─[:1:7]
       │
     1 │ Answer: T-H-A-T!
       │       ┬
       │       ╰── unexpected :
    ───╯
    "###);
}

#[test]
fn test_hint_missing_args() {
    assert_display_snapshot!(compile(r###"
    from film
    select [film_id, lag film_id]
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:3:22]
       │
     3 │     select [film_id, lag film_id]
       │                      ─────┬─────
       │                           ╰─────── function std.select, param `columns` expected type `column`, but found type `func infer -> column`
       │
       │ Help: Have you forgotten an argument to function std.lag?
    ───╯
    "###)
}
