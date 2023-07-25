use std::fs::read_to_string;

use insta::assert_snapshot;
use regex::Regex;
use similar::{ChangeTag, TextDiff};

#[test]
fn test_expr_ast_code_matches() {
    // expr.rs exists in both prql_ast as well as the prql_compiler crate (with the latter adding some fields/variants).
    // This test exists to ensure that the doc comments of the shared fields/variants stay in sync.
    assert_snapshot!(
        diff_code_after_start(
            &read_to_string("../../crates/prql-ast/src/expr.rs").unwrap(),
            &read_to_string("../../crates/prql-compiler/src/ir/pl/expr.rs").unwrap(),
        ), @r###"
    @@ .. @@
    -    pub return_ty: Option<Box<Expr>>,
    +    pub return_ty: Option<TyOrExpr>,
    @@ .. @@
    -    pub ty: Option<Box<Expr>>,
    +    pub ty: Option<TyOrExpr>,
    "###
    );
}

#[test]
fn test_stmt_ast_code_matches() {
    // stmt.rs exists in both prql_ast as well as the prql_compiler crate.
    // This test exists to ensure that the doc comments of the shared fields/variants stay in sync.
    assert_snapshot!(
        diff_code_after_start(
            &read_to_string("../../crates/prql-ast/src/stmt.rs").unwrap(),
            &read_to_string("../../crates/prql-compiler/src/ir/pl/stmt.rs").unwrap(),
        ), @r###"
    @@ .. @@
    -    Main(Box<Expr>),
    @@ .. @@
    -    pub name: String,
    +    pub name: Option<String>,
    "###
    )
}

fn diff_code_after_start(old: &str, new: &str) -> String {
    let divider_regex =
        Regex::new("// The following code is tested by the tests_misc crate .*\n").unwrap();
    let old = divider_regex.splitn(old, 2).nth(1).unwrap();
    let new = divider_regex.splitn(new, 2).nth(1).unwrap();
    diff_code(old, new)
}

/// Returns a unified diff of all diff hunks where some lines were removed.
fn diff_code(prql_ast_code: &str, pl_ast_code: &str) -> String {
    let mut diff = String::new();

    for hunk in TextDiff::from_lines(prql_ast_code, pl_ast_code)
        .unified_diff()
        .context_radius(0)
        .iter_hunks()
    {
        if hunk
            .iter_changes()
            .any(|change| change.tag() == ChangeTag::Delete)
            || !hunk
                .iter_changes()
                .any(|change| is_code(change.as_str().unwrap()))
        {
            diff.push_str(&hunk.to_string());
        }
    }

    // strip the line numbers since we don't want them in the snapshot
    let diff = Regex::new("@@ .* @@")
        .unwrap()
        .replace_all(&diff, "@@ .. @@");

    diff.to_string()
}

fn is_code(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with("//")
}

#[test]
fn test_diff_code() {
    assert_snapshot!(diff_code(
        "
    enum Enum {
        Foo,

        /// This comment will be changed
        Bar,

        /// This comment will be removed
        Baz,

        Fiz,
    }
    ",
        "
    enum Enum {
        Foo,

        /// This comment was changed
        Bar,

        Baz,

        /// This comment was added
        Fiz,

        /// This variant was added but won't show up in the diff since
        /// we only care about syncing comments of shared fields/variants.
        Buz,
    }
    "
    )
    , @r###"
    @@ .. @@
    -        /// This comment will be changed
    +        /// This comment was changed
    @@ .. @@
    -        /// This comment will be removed
    @@ .. @@
    +        /// This comment was added
    "###);
}

#[test]
fn test_diff_code_empty_line_isnt_code() {
    assert_snapshot!(diff_code(
        "
    enum Enum {
        /// This comment will be removed
        Baz,
        Fiz,
    }
    ",
        "
    enum Enum {
        Baz,

        /// This comment was added
        Fiz,
        /// This variant was added
        Buz,
    }
    "
    )
    , @r###"
    @@ .. @@
    -        /// This comment will be removed
    @@ .. @@
    +
    +        /// This comment was added
    "###);
}
