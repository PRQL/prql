use prqlc::pr::{ExprKind, Stmt, StmtKind, TyKind, VarDefKind};

/// Generate HTML documentation.
pub fn generate_html_docs(stmts: Vec<Stmt>) -> String {
    let html = format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="keywords" content="prql">
    <meta name="generator" content="prqlc {}">
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-T3c6CoIi6uLrA9TneNEoa7RxnatzjcDSCmG1MXxSR1GAsXEV/Dwwykc2MPK8M2HN" crossorigin="anonymous">
    <title>PRQL Docs</title>
  </head>
  <body>
    <header class="bg-body-tertiary">
      <div class="container">
        <h1>Documentation</h1>
      </div>
    </header>
    <main class="container">
      {{{{ content }}}}
    </main>
    <footer class="container border-top">
      <small class="text-body-secondary">Generated with <a href="https://prql-lang.org/" rel="external" target="_blank">prqlc</a> {}.</small>
    </footer>
  </body>
</html>
"#,
        prqlc::compiler_version(),
        prqlc::compiler_version()
    );

    let mut docs = String::new();

    docs.push_str("<h2>Functions</h2>\n");
    docs.push_str("<ul>\n");
    for stmt in stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
    {
        let var_def = stmt.kind.as_var_def().unwrap();
        docs.push_str(&format!(
            "  <li><a href=\"#fn-{}\">{}</a></li>\n",
            var_def.name, var_def.name
        ));
    }
    docs.push_str("</ul>\n\n");
    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::VarDef(_)))
        .count()
        == 0
    {
        docs.push_str("<p>None.</p>\n\n");
    }

    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
        .count()
        > 0
    {
        docs.push_str("<h2>Types</h2>\n");
        docs.push_str("<ul>\n");
        for stmt in stmts
            .clone()
            .into_iter()
            .filter(|stmt| matches!(stmt.kind, StmtKind::TypeDef(_)))
        {
            let type_def = stmt.kind.as_type_def().unwrap();
            if let Some(value) = &type_def.value {
                docs.push_str(&format!(
                    "  <li><code>{}</code> – {:?}</li>\n",
                    type_def.name, value.kind
                ));
            } else {
                docs.push_str(&format!("  <li>{}</li>\n", type_def.name));
            }
        }
        docs.push_str("</ul>\n");
    }

    if stmts
        .clone()
        .into_iter()
        .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
        .count()
        > 0
    {
        docs.push_str("<h2>Modules</h2>\n");
        docs.push_str("<ul>\n");
        for stmt in stmts
            .clone()
            .into_iter()
            .filter(|stmt| matches!(stmt.kind, StmtKind::ModuleDef(_)))
        {
            let module_def = stmt.kind.as_module_def().unwrap();
            docs.push_str(&format!("  <li>{}</li>\n", module_def.name));
        }
        docs.push_str("</ul>\n");
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

        docs.push_str("<section>\n");
        docs.push_str(&format!(
            "  <h3 id=\"fn-{}\">{}</h3>\n",
            var_def.name, var_def.name
        ));

        docs.push_str("<div class=\"ms-3\">\n");

        if let Some(doc_comment) = stmt.doc_comment {
            docs.push_str(&format!("  <p>{doc_comment}</p>\n"));
        }

        if let Some(expr) = &var_def.value {
            match &expr.kind {
                ExprKind::Func(func) => {
                    if !func.generic_type_params.is_empty() {
                        docs.push_str("  <h4 class=\"h6\">Type parameters</h4>\n");
                        docs.push_str("  <ul>\n");
                        for param in &func.generic_type_params {
                            docs.push_str(&format!("    <li><var>{}</var></li>\n", param.name));
                        }
                        docs.push_str("  </ul>\n");
                    }

                    if !func.params.is_empty() {
                        docs.push_str("  <h4 class=\"h6\">Parameters</h4>\n");
                        docs.push_str("  <ul>\n");
                        for param in &func.params {
                            docs.push_str(&format!("    <li><var>{}</var></li>\n", param.name));
                        }
                        docs.push_str("  </ul>\n");
                    }

                    if !func.named_params.is_empty() {
                        docs.push_str("  <h4 class=\"h6\">Named parameters</h4>\n");
                        docs.push_str("  <ul>\n");
                        for param in &func.named_params {
                            docs.push_str(&format!("    <li><var>{}</var></li>\n", param.name));
                        }
                        docs.push_str("  </ul>\n");
                    }

                    if let Some(return_ty) = &func.return_ty {
                        docs.push_str("  <h4 class=\"h6\">Returns</h4>\n");
                        match &return_ty.kind {
                            TyKind::Any => docs.push_str("  <p>Any</p>\n"),
                            TyKind::Ident(ident) => {
                                docs.push_str(&format!("  <p><code>{}</code></p>\n", ident.name));
                            }
                            TyKind::Primitive(primitive) => {
                                docs.push_str(&format!("  <p><code>{primitive}</code></p>\n"));
                            }
                            TyKind::Singleton(literal) => {
                                docs.push_str(&format!("  <p><code>{literal}</code></p>\n"));
                            }
                            TyKind::Union(vec) => {
                                docs.push_str("  <ul class=\"list-unstyled\">\n");
                                for (_, ty) in vec {
                                    docs.push_str(&format!("    <li>{:?}</li>\n", ty.kind));
                                }
                                docs.push_str("  </ul>\n");
                            }
                            _ => docs.push_str("  <p class=\"text-danger\">Not implemented</p>\n"),
                        }
                    }
                }
                ExprKind::Pipeline(_) => {
                    docs.push_str("  <p>There is a pipeline.</p>\n");
                }
                _ => (),
            }
        }

        docs.push_str("</div>\n");
        docs.push_str("</section>\n");
    }

    html.replacen("{{ content }}", &docs, 1)
}

/// Generate Markdown documentation.
pub fn generate_markdown_docs(stmts: Vec<Stmt>) -> String {
    let markdown = format!(
        r#"# Documentation

{{{{ content }}}}

Generated with [prqlc](https://prql-lang.org/) {}.
"#,
        prqlc::compiler_version()
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

        if let Some(doc_comment) = stmt.doc_comment {
            docs.push_str(&format!("{}\n", doc_comment.trim_start()));
        }
        docs.push('\n');

        if let Some(expr) = &var_def.value {
            match &expr.kind {
                ExprKind::Func(func) => {
                    if !func.generic_type_params.is_empty() {
                        docs.push_str("#### Type Parameters\n");
                        for param in &func.generic_type_params {
                            docs.push_str(&format!("* *{}*\n", param.name));
                        }
                        docs.push('\n');
                    }

                    if !func.params.is_empty() {
                        docs.push_str("#### Parameters\n");
                        for param in &func.params {
                            docs.push_str(&format!("* *{}*\n", param.name));
                        }
                        docs.push('\n');
                    }

                    if !func.named_params.is_empty() {
                        docs.push_str("#### Named parameters\n");
                        for param in &func.named_params {
                            docs.push_str(&format!("* *{}*\n", param.name));
                        }
                        docs.push('\n');
                    }

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
    use std::process::Command;

    use insta_cmd::assert_cmd_snapshot;
    use insta_cmd::get_cargo_bin;

    #[test]
    fn generate_html_docs() {
        std::env::set_var("PRQL_VERSION_OVERRIDE", env!("CARGO_PKG_VERSION"));

        let input = r"
        #! This is the x function.
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

        assert_cmd_snapshot!(prqlc_command().args(["experimental", "doc", "--format=html"]).pass_stdin(input), @r##"
        success: true
        exit_code: 0
        ----- stdout -----
        <!doctype html>
        <html lang="en">
          <head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <meta name="keywords" content="prql">
            <meta name="generator" content="prqlc 0.13.2">
            <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.2/dist/css/bootstrap.min.css" rel="stylesheet" integrity="sha384-T3c6CoIi6uLrA9TneNEoa7RxnatzjcDSCmG1MXxSR1GAsXEV/Dwwykc2MPK8M2HN" crossorigin="anonymous">
            <title>PRQL Docs</title>
          </head>
          <body>
            <header class="bg-body-tertiary">
              <div class="container">
                <h1>Documentation</h1>
              </div>
            </header>
            <main class="container">
              <h2>Functions</h2>
        <ul>
          <li><a href="#fn-x">x</a></li>
          <li><a href="#fn-fn_returns_array">fn_returns_array</a></li>
          <li><a href="#fn-fn_returns_bool">fn_returns_bool</a></li>
          <li><a href="#fn-fn_returns_float">fn_returns_float</a></li>
          <li><a href="#fn-fn_returns_int">fn_returns_int</a></li>
          <li><a href="#fn-fn_returns_null">fn_returns_null</a></li>
          <li><a href="#fn-fn_returns_text">fn_returns_text</a></li>
        </ul>

        <h2>Types</h2>
        <ul>
          <li><code>user_id</code> – Primitive(Int)</li>
        </ul>
        <h2>Modules</h2>
        <ul>
          <li>foo</li>
        </ul>
        <section>
          <h3 id="fn-x">x</h3>
        <div class="ms-3">
          <p> This is the x function.</p>
          <h4 class="h6">Parameters</h4>
          <ul>
            <li><var>arg1</var></li>
            <li><var>arg2</var></li>
          </ul>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_array">fn_returns_array</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>array</code></p>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_bool">fn_returns_bool</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>bool</code></p>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_float">fn_returns_float</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>float</code></p>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_int">fn_returns_int</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>int</code></p>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_null">fn_returns_null</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>null</code></p>
        </div>
        </section>
        <section>
          <h3 id="fn-fn_returns_text">fn_returns_text</h3>
        <div class="ms-3">
          <h4 class="h6">Returns</h4>
          <p><code>text</code></p>
        </div>
        </section>

            </main>
            <footer class="container border-top">
              <small class="text-body-secondary">Generated with <a href="https://prql-lang.org/" rel="external" target="_blank">prqlc</a> 0.13.2.</small>
            </footer>
          </body>
        </html>

        ----- stderr -----
        "##);
    }

    #[test]
    fn generate_markdown_docs() {
        std::env::set_var("PRQL_VERSION_OVERRIDE", env!("CARGO_PKG_VERSION"));

        let input = r"
        #! This is the x function.
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

        assert_cmd_snapshot!(prqlc_command().args(["experimental", "doc"]).pass_stdin(input), @r#####"
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
        This is the x function.

        #### Parameters
        * *arg1*
        * *arg2*


        ### fn_returns_array

        #### Returns
        `array`

        ### fn_returns_bool

        #### Returns
        `bool`

        ### fn_returns_float

        #### Returns
        `float`

        ### fn_returns_int

        #### Returns
        `int`

        ### fn_returns_null

        #### Returns
        `null`

        ### fn_returns_text

        #### Returns
        `text`



        Generated with [prqlc](https://prql-lang.org/) 0.13.2.

        ----- stderr -----
        "#####);
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
