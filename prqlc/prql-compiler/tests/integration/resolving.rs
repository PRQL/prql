use anyhow::Result;
use insta::assert_snapshot;
use prql_compiler::ErrorMessages;

// equivalent to prqlc debug resolve
fn resolve(prql_source: &str) -> Result<String, ErrorMessages> {
    let sources = prql_compiler::SourceTree::single("".into(), prql_source.to_string());
    let stmts = prql_compiler::prql_to_pl_tree(&sources)?;

    let root_module = prql_compiler::semantic::resolve(stmts, Default::default())
        .map_err(prql_compiler::downcast)
        .map_err(|e| e.composed(&sources))?;

    // resolved PL, restricted back into AST
    let mut root_module = prql_compiler::semantic::ast_expand::restrict_module(root_module.module);
    drop_module_defs(&mut root_module.stmts, &["std", "default_db"]);

    prql_compiler::pl_to_prql(root_module.stmts)
}

fn drop_module_defs(stmts: &mut Vec<prqlc_ast::stmt::Stmt>, to_drop: &[&str]) {
    stmts.retain(|x| {
        x.kind
            .as_module_def()
            .map_or(true, |m| !to_drop.contains(&m.name.as_str()))
    });
}

#[test]
fn resolve_basic() {
    assert_snapshot!(resolve(r#"
    from x
    select {a, b}
    "#).unwrap(), @r###"
    let main <[{a = ?, b = ?}]> = `(Select ...)`
    "###)
}

#[test]
fn resolve_types_01() {
    assert_snapshot!(resolve(r#"
    type A = int || int
    "#).unwrap(), @r###"
    type A = int
    "###)
}

#[test]
fn resolve_types_02() {
    assert_snapshot!(resolve(r#"
    type A = int || ()
    "#).unwrap(), @r###"
    type A = int
    "###)
}

#[test]
fn resolve_types_03() {
    assert_snapshot!(resolve(r#"
    type A = {a = int, bool} || {b = text, float}
    "#).unwrap(), @r###"
    type A = {a = int, bool, b = text, float}
    "###)
}

#[test]
fn resolve_types_04() {
    assert_snapshot!(resolve(
        r#"
    type Status = (
        Paid = () ||
        Unpaid = float ||
        Canceled = {reason = text, cancelled_at = timestamp} ||
    )
    "#,
    )
    .unwrap(), @r###"
    type Status = (
      Paid = () ||
      Unpaid = float ||
      {reason = text, cancelled_at = timestamp} ||
    )
    "###);
}
