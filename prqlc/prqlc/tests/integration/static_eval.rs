use insta::assert_yaml_snapshot;
use prqlc::ir::{constant::ConstExpr, decl::RootModule};
use prqlc_parser::ast::{Expr, VarDef};

#[track_caller]
fn static_eval(prql_source: &str) -> ConstExpr {
    let sources = prqlc::SourceTree::single("".into(), prql_source.to_string());
    let stmts_tree = prqlc::prql_to_pl_tree(&sources).unwrap();

    let stmt = &stmts_tree.stmts[0];
    let var_def: &VarDef = stmt.kind.as_var_def().unwrap();
    let expr: Expr = *var_def.value.as_ref().unwrap().clone();

    let expr = prqlc::semantic::ast_expand::expand_expr(expr).unwrap();

    let mut root_module = RootModule::default();

    prqlc::semantic::static_eval(expr, &mut root_module)
        .map_err(|e| prqlc::ErrorMessages::from(e).composed(&sources))
        .unwrap()
}

#[test]
fn basic_01() {
    assert_yaml_snapshot!(static_eval(r#"
    {
        a = 1, "hello", [1.1, 0.0, 2.4], {null, false}
    }
    "#), @r###"
    ---
    kind:
      Tuple:
        - kind:
            Literal:
              Integer: 1
          span: "1:19-20"
        - kind:
            Literal:
              String: hello
          span: "1:22-29"
        - kind:
            Array:
              - kind:
                  Literal:
                    Float: 1.1
                span: "1:32-35"
              - kind:
                  Literal:
                    Float: 0
                span: "1:37-40"
              - kind:
                  Literal:
                    Float: 2.4
                span: "1:42-45"
          span: "1:31-46"
        - kind:
            Tuple:
              - kind:
                  Literal: "Null"
                span: "1:49-53"
              - kind:
                  Literal:
                    Boolean: false
                span: "1:55-60"
          span: "1:48-61"
    span: "1:5-67"
    "###)
}
