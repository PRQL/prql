use insta::assert_snapshot;
use prqlc::ErrorMessages;
use prqlc_parser::parser::pr;

// equivalent to prqlc debug resolve
fn resolve(prql_source: &str) -> Result<String, ErrorMessages> {
    let sources = prqlc::SourceTree::single("".into(), prql_source.to_string());
    let stmts = prqlc::prql_to_pl_tree(&sources)?;

    let root_module = prqlc::semantic::resolve(stmts)
        .map_err(|e| prqlc::ErrorMessages::from(e).composed(&sources))?;

    // resolved PL, restricted back into AST
    let mut root_module = prqlc::semantic::ast_expand::restrict_module(root_module.module);
    drop_module_defs(&mut root_module.stmts, &["std", "default_db"]);

    prqlc::pl_to_prql(&root_module)
}

fn drop_module_defs(stmts: &mut Vec<pr::Stmt>, to_drop: &[&str]) {
    stmts.retain(|x| {
        x.kind
            .as_module_def()
            .map_or(true, |m| !to_drop.contains(&m.name.as_str()))
    });
}

#[test]
fn resolve_basic_01() {
    assert_snapshot!(resolve(r#"
    from x
    select {a, b}
    "#).unwrap(), @"let main <[{a = ?, b = ?}]> = `(Select ...)`")
}

#[test]
fn resolve_function_01() {
    assert_snapshot!(resolve(r#"
    let my_func = func param_1 <param_1_type> -> <Ret_ty> (
      param_1 + 1
    )
    "#).unwrap(), @r"
    let my_func = func param_1 <param_1_type> -> <Ret_ty> (
      std.add param_1 1
    )
    ")
}
