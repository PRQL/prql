use prqlc::{prql_to_pl, ErrorMessages};
use prqlc_ast::{stmt::StmtKind, ExprKind, Stmt, TyKind, VarDefKind};

// pub fn generate_docs(prql: &str) -> Result<String, ErrorMessages> {
//     let pl = prql_to_pl(prql);
//     if let Err(e) = pl {
//         return Err(e);
//     }

//     Ok(generate_docs_stmt(pl.unwrap()))
// }

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
        *prqlc::COMPILER_VERSION,
        *prqlc::COMPILER_VERSION
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

        //if let Some(docComment) = vardef.DocComment {
        //    docs.push_str(&format!("  <p>{docComment}</p>\n"));
        //}

        if let Some(expr) = &var_def.value {
            match &expr.kind {
                ExprKind::Func(boxfn) => {
                    docs.push_str("  <h4 class=\"h6\">Parameters</h4>\n");
                    docs.push_str("  <ul>\n");
                    for param in &boxfn.params {
                        docs.push_str(&format!("    <li><var>{}</var></li>\n", param.name));
                    }
                    docs.push_str("  </ul>\n");
                }
                _ => (),
            }

            match &expr.kind {
                ExprKind::Func(func) => {
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
                _ => (),
            }
        }

        docs.push_str("</section>\n");
    }

    html.replacen("{{ content }}", &docs, 1)
}
