use std::fs::read_to_string;

use insta::assert_snapshot;
use regex::Regex;
use similar::{ChangeTag, TextDiff};

#[test]
fn test_expr_ast_code_matches() {
    // expr.rs exists in both prqlc_ast as well as the prql_compiler crate (with the latter adding some fields/variants).
    // This test exists to ensure that the doc comments of the shared fields/variants stay in sync.
    assert_snapshot!(
        diff_code_after_start(
            &read_to_string("../prqlc-parser/src/parser/pr/expr.rs").unwrap(),
            &read_to_string("../prqlc/src/ir/pl/expr.rs").unwrap(),
        ), @r###"
    @@ .. @@
    -#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    +#[derive(Clone, PartialEq, Serialize, Deserialize)]
    @@ .. @@
    -    Ident(String),
    -    Indirection {
    -        base: Box<Expr>,
    -        field: IndirectionKind,
    +    Ident(Ident),
    +    All {
    +        within: Box<Expr>,
    +        except: Box<Expr>,
    @@ .. @@
    -    #[cfg_attr(
    -        feature = "serde_yaml",
    -        serde(with = "serde_yaml::with::singleton_map")
    -    )]
    -    Literal(token::Literal),
    -    Pipeline(Pipeline),
    +    Literal(Literal),
    @@ .. @@
    -    Range(Range),
    -    Binary(BinaryExpr),
    -    Unary(UnaryExpr),
    @@ .. @@
    -#[derive(Debug, EnumAsInner, PartialEq, Clone, Serialize, Deserialize)]
    -pub enum IndirectionKind {
    -    Name(String),
    -    Position(i64),
    -    Star,
    -}
    -
    -#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
    -pub struct BinaryExpr {
    -    pub left: Box<Expr>,
    -    pub op: BinOp,
    -    pub right: Box<Expr>,
    -}
    -
    -#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
    -pub struct UnaryExpr {
    -    pub op: UnOp,
    -    pub expr: Box<Expr>,
    -}
    -
    @@ .. @@
    -#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
    -pub struct GenericTypeParam {
    -    /// Assigned name of this generic type argument.
    -    pub name: String,
    -
    -    pub domain: Vec<Ty>,
    -}
    -
    -/// A value and a series of functions that are to be applied to that value one after another.
    -#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
    -pub struct Pipeline {
    -    pub exprs: Vec<Expr>,
    -}
    -
    @@ .. @@
    -impl From<token::Literal> for ExprKind {
    -    fn from(value: token::Literal) -> Self {
    +impl From<Literal> for ExprKind {
    +    fn from(value: Literal) -> Self {
    "###
    );
}

#[test]
fn test_stmt_ast_code_matches() {
    // stmt.rs exists in both prqlc_ast as well as the prql_compiler crate.
    // This test exists to ensure that the doc comments of the shared fields/variants stay in sync.
    assert_snapshot!(
        diff_code_after_start(
            &read_to_string("../prqlc-parser/src/parser/pr/stmt.rs").unwrap(),
            &read_to_string("../prqlc/src/ir/pl/stmt.rs").unwrap(),
        ), @r###"
    @@ .. @@
    -    pub kind: VarDefKind,
    @@ .. @@
    -}
    -
    -impl Stmt {
    -    pub fn new(kind: StmtKind) -> Stmt {
    -        Stmt {
    -            kind,
    -            span: None,
    -            annotations: Vec::new(),
    -        }
    -    }
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
fn diff_code(prqlc_ast_code: &str, pl_ast_code: &str) -> String {
    let mut diff = String::new();

    for hunk in TextDiff::from_lines(prqlc_ast_code, pl_ast_code)
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
