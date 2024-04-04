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
    drop_irrelevant_stuff(
        &mut root_module.stmts,
        &["std", "_local", "_infer", "_infer_module", "_generic"],
    );

    prqlc::pl_to_prql(&root_module)
}

fn drop_irrelevant_stuff(stmts: &mut Vec<prqlc_ast::stmt::Stmt>, to_drop: &[&str]) {
    stmts.retain_mut(|x| {
        match &mut x.kind {
            prqlc_ast::StmtKind::ModuleDef(m) => {
                if to_drop.contains(&m.name.as_str()) {
                    return false;
                }

                drop_irrelevant_stuff(&mut m.stmts, to_drop);
            }
            prqlc_ast::StmtKind::VarDef(v) => {
                if to_drop.contains(&v.name.as_str()) {
                    return false;
                }
            }
            _ => (),
        }
        true
    });
}

#[test]
fn resolve_basic_01() {
    assert_snapshot!(resolve(r#"
    module db {
        let x <[{a = int, b = text, c = float}]>
    }

    from db.x
    select {a, b}
    "#).unwrap(), @r###"
    module db {
      let x <[{a = int, b = text, c = float}]> = internal local_table
    }

    let main <[{a = int, b = text}]> = `(Select ...)`
    "###)
}

#[test]
fn resolve_ty_tuple_unpack() {
    assert_snapshot!(resolve(r#"
    type Employee = {first_name = text, age = int}

    let employees <[{ id = int, ..module.Employee }]>
    "#).unwrap(), @r###"
    type Employee = {first_name = text, age = int}

    module db {
    }

    let employees <[{id = int, first_name = text, age = int}]> = internal local_table
    "###)
}

#[test]
fn resolve_ty_exclude() {
    assert_snapshot!(resolve(r#"
    type X = {a = int, b = text}
    type Y = {b = text}
    type Z = module.X - module.Y
    "#).unwrap(), @r###"
    type X = {a = int, b = text}

    type Y = {b = text}

    type Z = {a = int}

    module db {
    }
    "###);
    
    assert_snapshot!(resolve(r#"
    type X = {a = int, b = text}
    type Y = text
    type Z = module.X - module.Y
    "#).unwrap_err(), @r###"
    Error:
       ╭─[:4:25]
       │
     4 │     type Z = module.X - module.Y
       │                         ────┬───
       │                             ╰───── expected excluding fields to be a tuple
       │
       │ Help: got text
    ───╯
    "###);
    
    assert_snapshot!(resolve(r#"
    type X = text
    type Y = {a = int}
    type Z = module.X - module.Y
    "#).unwrap_err(), @r###"
    Error:
       ╭─[:4:14]
       │
     4 │     type Z = module.X - module.Y
       │              ────┬───
       │                  ╰───── fields can only be excluded from a tuple
       │
       │ Help: got text
    ───╯
    "###);
}

#[test]
fn resolve_function_01() {
    assert_snapshot!(resolve(r#"
    let my_func = func param_1 <param_1_type> -> <Ret_ty> (
      param_1 + 1
    )
    "#).unwrap(), @r###"
    module db {
    }

    let my_func = func param_1 <param_1_type> -> <Ret_ty> std.add param_1 1
    "###)
}

#[test]
fn resolve_generics_01() {
    assert_snapshot!(resolve(
        r#"
    let add_one = func <A: int | float> a <A> -> <A> a + 1
        
    let my_int = module.add_one 1
    let my_float = module.add_one 1.0
    "#,
    )
    .unwrap(), @r###"
    let add_one = func <A: int | float> a <A> -> <A> (
      std.add a 1
    )

    module db {
    }

    let my_float <float> = `(std.add ...)`

    let my_int <int> = `(std.add ...)`
    "###);
}

#[test]
fn table_inference_01() {
    assert_snapshot!(resolve(
        r#"
    from db.employees
    "#,
    )
    .unwrap(), @r###"
    module db {
      let employees <[{.._generic.G109}]> = internal local_table
    }

    let main <[{.._generic.G109}]> = db.employees
    "###);
}

#[test]
fn table_inference_02() {
    assert_snapshot!(resolve(
        r#"
    from db.employees
    select {id, age}
    "#,
    )
    .unwrap(), @r###"
    module db {
      let employees <[{.._generic.G112}]> = internal local_table
    }

    let main <[{id = _generic.G118, age = _generic.G122}]> = `(Select ...)`
    "###);
}

#[test]
fn table_inference_03() {
    assert_snapshot!(resolve(
        r#"
    from db.employees
    select {e = this}
    select {e.name}
    "#,
    )
    .unwrap(), @r###"
    module db {
      let employees <[{.._generic.G115}]> = internal local_table
    }

    let main <[{name = _generic.G125}]> = `(Select ...)`
    "###);
}
