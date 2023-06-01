//! Test error messages. As of 2023-03, this can hopefully be expanded significantly.
//! It's also fine to put errors by the things that they're testing.
//! See also [test_bad_error_messages.rs](test_bad_error_messages.rs) for error
//! messages which need to be improved.

use super::compile;
use insta::assert_display_snapshot;

#[test]
fn test_errors() {
    assert_display_snapshot!(compile(r###"
    let addadd = a b -> a + b

    from x
    derive y = (addadd 4 5 6)
    "###).unwrap_err(),
        @r###"
    Error:
       ╭─[:5:17]
       │
     5 │     derive y = (addadd 4 5 6)
       │                 ──────┬─────
       │                       ╰─────── Too many arguments to function `addadd`
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
       │     ───────┬───────
       │            ╰───────── Too many arguments to function `from`
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
       │       ╰── unexpected : while parsing source file
    ───╯
    "###);
}

#[test]
fn test_union_all_sqlite() {
    // TODO: `SQLiteDialect` would be better as `sql.sqlite` or `sqlite`.
    assert_display_snapshot!(compile(r###"
    prql target:sql.sqlite

    from film
    remove film2
    "###).unwrap_err(), @r###"
    Error: The dialect SQLiteDialect does not support EXCEPT ALL
    ↳ Hint: Providing more column information will allow the query to be translated to an anti-join.
    "###)
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
       │                           ╰─────── function std.select, param `columns` expected type `scalar`, but found type `infer -> array`
       │
       │ Help: Have you forgotten an argument to function std.lag?
    ───╯
    "###)
}

#[test]
fn test_regex_dialect() {
    assert_display_snapshot!(compile(r###"
    prql target:sql.mssql
    from foo
    filter bar ~= 'love'
    "###).unwrap_err(), @r###"
    Error:
       ╭─[:4:12]
       │
     4 │     filter bar ~= 'love'
       │            ──────┬──────
       │                  ╰──────── regex functions are not supported by MsSql
    ───╯
    "###)
}
