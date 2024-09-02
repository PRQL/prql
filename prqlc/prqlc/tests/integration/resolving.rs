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
    drop_irrelevant_stuff(
        &mut root_module.stmts,
        &["std", "_local", "_infer", "_generic"],
    );

    prqlc::pl_to_prql(&root_module)
}

fn drop_irrelevant_stuff(stmts: &mut Vec<pr::Stmt>, to_drop: &[&str]) {
    stmts.retain_mut(|x| {
        match &mut x.kind {
            pr::StmtKind::ModuleDef(m) => {
                if to_drop.contains(&m.name.as_str()) {
                    return false;
                }

                drop_irrelevant_stuff(&mut m.stmts, to_drop);
            }
            pr::StmtKind::VarDef(v) => {
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
      let x <[{a = int, b = text, c = float}]> = $x
    }

    let main <[{a = int, b = text}]> = std.select {this.0, this.1} (
      std.from db.x
    )
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

    let employees <[{id = int, first_name = text, age = int}]> = $employees
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
       │                             ╰───── expected excluded fields to be a tuple
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
#[ignore]
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
#[ignore]
fn resolve_generics_02() {
    assert_snapshot!(resolve(
    r#"
    let neg = func <T> num<T> -> <T> -num
    let map = func <E, M>
        mapper <func E -> M>
        elements <[E]>
        -> <[M]> s"an array of mapped elements"

    let ints = [1, 2, 3]
    let negated = (module.map module.neg module.ints)
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
      let employees <[{.._generic.T0}]> = $
    }

    let main <[{..T0}]> = std.from db.employees
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
      let employees <[{.._generic.T0}]> = $
    }

    let main <[{id = F378, age = F381}]> = std.select {this.0, this.1} (
      std.from db.employees
    )
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
      let employees <[{.._generic.T0}]> = $
    }

    let main <[{name = F385}]> = std.select {this.0.0} (
      std.select {e = this} (std.from db.employees)
    )
    "###);
}

#[test]
fn table_inference_04() {
    assert_snapshot!(resolve(
        r#"
    let len_of_ints = func arr<[int]> -> <int> 0

    module.len_of_ints []
    "#,
    )
    .unwrap(), @r###"
    module db {
    }

    let len_of_ints <func [int] -> int> = func arr <[int]> -> <int> 0

    let main <int> = len_of_ints []
    "###);
}

#[test]
#[ignore]
fn high_order_func_01() {
    assert_snapshot!(resolve(
    r#"
    let neg = func <T> num<T> -> <T> -num
    let map = func <E, M>
        mapper <func E -> M>
        elements <[E]>
        -> <[M]> s"an array of mapped elements"
    
    let ints = [1, 2, 3]
    let negated = (module.map module.neg module.ints)
    "#,
    )
    .unwrap(), @r###"
    module db {
    }

    let ints <[int]> = [1, 2, 3]

    let map = func <E, M> mapper <func E -> M> elements <[E]> -> <[M]> s"an array of mapped elements"

    let neg = func <T> num <T> -> <T> std.neg num

    let negated <[int]> = s"an array of mapped elements"
    "###);
}

#[test]
#[ignore]
fn high_order_func_02() {
    assert_snapshot!(resolve(
    r#"
    let convert_to_one = func <X> x <X> -> <int> 1

    let both = func <I, O>
      mapper <func I -> O>
      input <{I, I}>
      -> <{O, O}> {
        (mapper input.0),
        (mapper input.1),
      }
    
    let both_ones = (module.both module.convert_to_one)

    let main = (module.both_ones {'hello', 'world'})
    "#,
    )
    .unwrap(), @r###"
    let both = func <I, O> mapper <func I -> O> input <{I, I}> -> <{O, O}> {
      mapper input.0,
      mapper input.1,
    }

    let both_ones <func {I, I} -> {O, O}> = (
      func <I, O> mapper <func I -> O> input <{I, I}> -> <{O, O}> {
        mapper input.0,
        mapper input.1,
      }
    ) convert_to_one

    let convert_to_one = func <X> x <X> -> <int> 1

    module db {
    }

    let main <{int, int}> = {1, 1}
    "###);
}
