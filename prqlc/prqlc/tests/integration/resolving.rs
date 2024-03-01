use insta::assert_snapshot;
use prqlc::ErrorMessages;

// equivalent to prqlc debug resolve
fn resolve(prql_source: &str) -> Result<String, ErrorMessages> {
    let sources = prqlc::SourceTree::single("".into(), prql_source.to_string());
    let stmts = prqlc::prql_to_pl_tree(&sources)?;

    let root_module = prqlc::semantic::resolve(stmts, Default::default())
        .map_err(|e| prqlc::ErrorMessages::from(e).composed(&sources))?;

    // resolved PL, restricted back into AST
    let mut root_module = prqlc::semantic::ast_expand::restrict_module(root_module.module);
    drop_module_defs(&mut root_module.stmts, &["std", "db"]);

    prqlc::pl_to_prql(&root_module)
}

fn drop_module_defs(stmts: &mut Vec<prqlc_ast::stmt::Stmt>, to_drop: &[&str]) {
    stmts.retain(|x| {
        x.kind
            .as_module_def()
            .map_or(true, |m| !to_drop.contains(&m.name.as_str()))
    });
}

#[test]
fn resolve_basic_01() {
    assert_snapshot!(resolve(r#"
    from db.x
    select {a, b}
    "#).unwrap(), @r###"
    let main <[{a = ?, b = ?}]> = `(Select ...)`
    "###)
}

#[test]
fn resolve_function_01() {
    assert_snapshot!(resolve(r#"
    let my_func = func param_1 <param_1_type> -> <Ret_ty> (
      param_1 + 1
    )
    "#).unwrap(), @r###"
    let my_func = func param_1 <param_1_type> -> <Ret_ty> (
      std.add param_1 1
    )
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

#[test]
fn resolve_types_05() {
    // TODO: this is very strange, it should only be allowed in std
    assert_snapshot!(resolve(
        r#"
    type A
    "#,
    )
    .unwrap(), @r###"
    type A = null
    "###);
}

#[test]
fn resolve_generics_01() {
    assert_snapshot!(resolve(
        r#"
    let add_one = func <A: int | float> a <A> -> <A> a + 1
        
    let my_int = add_one 1
    let my_float = add_one 1.0
    "#,
    )
    .unwrap(), @r###"
    let add_one = func <A: int | float> a <A> -> <A> (
      std.add a 1
    )

    let my_float <float> = `(std.add ...)`

    let my_int <int> = `(std.add ...)`
    "###);
}
