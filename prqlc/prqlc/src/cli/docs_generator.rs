use prqlc_ast::{stmt::StmtKind, ExprKind, Stmt, TyKind, VarDefKind};

/// Generate HTML documentation.
// pub fn generate_html_docs(stmts: Vec<Stmt>) -> String {
//     let html = format!(
//         r#"<!doctype html>
// <html lang="en">
//   <head>
//     <meta charset="utf-8">
//     <meta name="viewport" content="width=device-width, initial-scale=1">
//     <meta name="keywords" content="prql">
//     <meta name="generator" content="prqlc {}">
//     <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-T3c6CoIi6uLrA9TneNEoa7RxnatzjcDSCmG1MXxSR1GAsXEV/Dwwykc2MPK8M2HN" crossorigin="anonymous">
//     <title>PRQL Docs</title>
//   </head>
//   <body>
//     <header class="bg-body-tertiary">
//       <div class="container">
//         <h1>Documentation</h1>
//       </div>
//     </header>
//     <main class="container">
//       {{{{ content }}}}
//     </main>
//     <footer class="container border-top">
//       <small class="text-body-secondary">Generated with <a href="https://prql-lang.org/" rel="external" target="_blank">prqlc</a> {}.</small>
//     </footer>
//   </body>
// </html>
// "#,
//         *prqlc::COMPILER_VERSION,
//         *prqlc::COMPILER_VERSION
//     );

//     let mut docs = String::new();

//     docs.push_str("<h2>Functions</h2>\n");
//     docs.push_str("<ul>\n");
//     for stmt in stmts
//         .clone()
//         .into_iter()
//         .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
//     {
//         let var_def = stmt.kind.as_var_def().unwrap();
//         docs.push_str(&format!(
//             "  <li><a href=\"#fn-{}\">{}</a></li>\n",
//             var_def.name, var_def.name
//         ));
//     }
//     docs.push_str("</ul>\n\n");
//     if stmts
//         .clone()
//         .into_iter()
//         .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
//         .count()
//         == 0
//     {
//         docs.push_str("<p>None.</p>\n\n");
//     }

//     if stmts
//         .clone()
//         .into_iter()
//         .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
//         .count()
//         > 0
//     {
//         docs.push_str("<h2>Types</h2>\n");
//         docs.push_str("<ul>\n");
//         for stmt in stmts
//             .clone()
//             .into_iter()
//             .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
//         {
//             let type_def = stmt.kind.as_type_def().unwrap();
//             if let Some(value) = &type_def.value {
//                 docs.push_str(&format!(
//                     "  <li><code>{}</code> – {:?}</li>\n",
//                     type_def.name, value.kind
//                 ));
//             } else {
//                 docs.push_str(&format!("  <li>{}</li>\n", type_def.name));
//             }
//         }
//         docs.push_str("</ul>\n");
//     }

//     if stmts
//         .clone()
//         .into_iter()
//         .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
//         .count()
//         > 0
//     {
//         docs.push_str("<h2>Modules</h2>\n");
//         docs.push_str("<ul>\n");
//         for stmt in stmts
//             .clone()
//             .into_iter()
//             .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
//         {
//             let module_def = stmt.kind.as_module_def().unwrap();
//             docs.push_str(&format!("  <li>{}</li>\n", module_def.name));
//         }
//         docs.push_str("</ul>\n");
//     }

//     for stmt in stmts
//         .clone()
//         .into_iter()
//         .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
//     {
//         let var_def = stmt.kind.as_var_def().unwrap();
//         if var_def.kind != VarDefKind::Let {
//             continue;
//         }

//         docs.push_str("<section>\n");
//         docs.push_str(&format!(
//             "  <h3 id=\"fn-{}\">{}</h3>\n",
//             var_def.name, var_def.name
//         ));

//         //if let Some(docComment) = vardef.DocComment {
//         //    docs.push_str(&format!("  <p>{docComment}</p>\n"));
//         //}

//         if let Some(expr) = &var_def.value {
//             match &expr.kind {
//                 ExprKind::Func(func) => {
//                     docs.push_str("  <h4 class=\"h6\">Parameters</h4>\n");
//                     docs.push_str("  <ul>\n");
//                     for param in &func.params {
//                         docs.push_str(&format!("    <li><var>{}</var></li>\n", param.name));
//                     }
//                     docs.push_str("  </ul>\n");

//                     if let Some(return_ty) = &func.return_ty {
//                         docs.push_str("  <h4 class=\"h6\">Returns</h4>\n");
//                         match &return_ty.kind {
//                             TyKind::Any => docs.push_str("  <p>Any</p>\n"),
//                             TyKind::Ident(ident) => {
//                                 docs.push_str(&format!("  <p><code>{}</code></p>\n", ident.name));
//                             }
//                             TyKind::Primitive(primitive) => {
//                                 docs.push_str(&format!("  <p><code>{primitive}</code></p>\n"));
//                             }
//                             TyKind::Singleton(literal) => {
//                                 docs.push_str(&format!("  <p><code>{literal}</code></p>\n"));
//                             }
//                             TyKind::Union(vec) => {
//                                 docs.push_str("  <ul class=\"list-unstyled\">\n");
//                                 for (_, ty) in vec {
//                                     docs.push_str(&format!("    <li>{:?}</li>\n", ty.kind));
//                                 }
//                                 docs.push_str("  </ul>\n");
//                             }
//                             _ => docs.push_str("  <p class=\"text-danger\">Not implemented</p>\n"),
//                         }
//                     }
//                 }
//                 ExprKind::Pipeline(_) => {
//                     docs.push_str("  <p>There is a pipeline.</p>\n");
//                 }
//                 _ => (),
//             }
//         }

//         docs.push_str("</section>\n");
//     }

//     html.replacen("{{ content }}", &docs, 1)
// }

/// Generate Markdown documentation.
pub fn generate_markdown_docs(stmts: Vec<Stmt>) -> String {
    let markdown = format!(
        r#"# Documentation

{{{{ content }}}}

Generated with [prqlc](https://prql-lang.org/) {}.
"#,
        *prqlc::COMPILER_VERSION
    );

    let mut docs = String::new();

    docs.push_str("## Functions\n");
    for stmt in stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
    {
        let var_def = stmt.kind.as_var_def().unwrap();
        docs.push_str(&format!("* [{}](#{})\n", var_def.name, var_def.name));
    }
    docs.push('\n');

    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
        .count()
        == 0
    {
        docs.push_str("None.\n\n");
    }

    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
        .count()
        > 0
    {
        docs.push_str("## Types\n");
        for stmt in stmts
            .clone()
            .into_iter()
            .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
        {
            let type_def = stmt.kind.as_type_def().unwrap();
            if let Some(value) = &type_def.value {
                docs.push_str(&format!("* `{}` – {:?}\n", type_def.name, value.kind));
            } else {
                docs.push_str(&format!("* {}\n", type_def.name));
            }
        }
        docs.push('\n');
    }

    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
        .count()
        > 0
    {
        docs.push_str("## Modules\n");
        for stmt in stmts
            .clone()
            .into_iter()
            .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
        {
            let module_def = stmt.kind.as_module_def().unwrap();
            docs.push_str(&format!("* {}\n", module_def.name));
        }
        docs.push('\n');
    }

    for stmt in stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
    {
        let var_def = stmt.kind.as_var_def().unwrap();
        if var_def.kind != VarDefKind::Let {
            continue;
        }

        docs.push_str(&format!("### {}\n", var_def.name));

        //if let Some(docComment) = vardef.DocComment {
        //    docs.push_str(&format!("{docComment}\n"));
        //}
        docs.push('\n');

        if let Some(expr) = &var_def.value {
            match &expr.kind {
                ExprKind::Func(func) => {
                    docs.push_str("#### Parameters\n");
                    for param in &func.params {
                        docs.push_str(&format!("* *{}*\n", param.name));
                    }
                    docs.push('\n');

                    if let Some(return_ty) = &func.return_ty {
                        docs.push_str("#### Returns\n");
                        match &return_ty.kind {
                            TyKind::Any => docs.push_str("Any\n"),
                            TyKind::Ident(ident) => {
                                docs.push_str(&format!("`{}`\n", ident.name));
                            }
                            TyKind::Primitive(primitive) => {
                                docs.push_str(&format!("`{primitive}`\n"));
                            }
                            TyKind::Singleton(literal) => {
                                docs.push_str(&format!("`{literal}`\n"));
                            }
                            TyKind::Union(vec) => {
                                for (_, ty) in vec {
                                    docs.push_str(&format!("* {:?}\n", ty.kind));
                                }
                            }
                            _ => docs.push_str("Not implemented\n"),
                        }
                    }
                    docs.push('\n');
                }
                ExprKind::Pipeline(_) => {
                    docs.push_str("There is a pipeline.\n");
                }
                _ => (),
            }
        }
    }

    markdown.replacen("{{ content }}", &docs, 1)
}

#[cfg(test)]
mod tests {
    use insta_cmd::assert_cmd_snapshot;
    use insta_cmd::get_cargo_bin;
    use std::process::Command;

    #[test]
    fn generate_markdown_docs() {
        let input = r"
        let x = arg1 arg2 -> c
        let fn_returns_array = -> <array> array
        let fn_returns_bool = -> <bool> true
        let fn_returns_float = -> <float> float
        let fn_returns_int = -> <int> 0
        let fn_returns_null = -> <null> null
        let fn_returns_text = -> <text> 'text'

        module foo {}

        type user_id = int
        ";

        assert_cmd_snapshot!(prqlc_command().args(["experimental", "doc"]).pass_stdin(input), @r###"
        success: true
        exit_code: 0
        ----- stdout -----
        # Documentation

        ## Functions
        * [x](#x)
        * [fn_returns_array](#fn_returns_array)
        * [fn_returns_bool](#fn_returns_bool)
        * [fn_returns_float](#fn_returns_float)
        * [fn_returns_int](#fn_returns_int)
        * [fn_returns_null](#fn_returns_null)
        * [fn_returns_text](#fn_returns_text)

        ## Types
        * `user_id` – Primitive(Int)

        ## Modules
        * foo

        ### x

        #### Parameters
        * *arg1*
        * *arg2*


        ### fn_returns_array

        #### Parameters

        #### Returns
        `array`

        ### fn_returns_bool

        #### Parameters

        #### Returns
        `bool`

        ### fn_returns_float

        #### Parameters

        #### Returns
        `float`

        ### fn_returns_int

        #### Parameters

        #### Returns
        `int`

        ### fn_returns_null

        #### Parameters

        #### Returns
        `null`

        ### fn_returns_text

        #### Parameters

        #### Returns
        `text`



        Generated with [prqlc](https://prql-lang.org/) 0.11.5.

        ----- stderr -----
        "###);
    }

    fn prqlc_command() -> Command {
        let mut cmd = Command::new(get_cargo_bin("prqlc"));
        normalize_prqlc(&mut cmd);
        cmd
    }

    fn normalize_prqlc(cmd: &mut Command) -> &mut Command {
        cmd
            // We set `CLICOLOR_FORCE` in CI to force color output, but we don't want `prqlc` to
            // output color for our snapshot tests. And it seems to override the
            // `--color=never` flag.
            .env_remove("CLICOLOR_FORCE")
            .env("NO_COLOR", "1")
            .args(["--color=never"])
            // We don't want the tests to be affected by the user's `RUST_BACKTRACE` setting.
            .env_remove("RUST_BACKTRACE")
            .env_remove("RUST_LOG")
    }
}
