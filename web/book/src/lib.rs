// All copied from `mdbook_preprocessor_boilerplate` apart from the function
// which does the replacement.
// This file is licensed under GPL-3.0 then. We don't link against it from PRQL.

#![cfg(not(target_family = "wasm"))]

use anyhow::{bail, Result};
use clap::{Arg, ArgMatches, Command};
use mdbook::preprocess::PreprocessorContext;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook::{book::Book, BookItem};
use prql_compiler::compile;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use pulldown_cmark_to_cmark::cmark;
use semver::{Version, VersionReq};
use std::{io, process};

/// Checks renderer support and runs the preprocessor.
pub fn run(preprocessor: impl Preprocessor, name: &'static str, description: &'static str) {
    let matches = Command::new(name)
        .about(description)
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
        .get_matches();

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(preprocessor) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn handle_preprocessing(pre: impl Preprocessor) -> Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    let out = serde_json::to_string(&processed_book)?;
    println!("{}", out);

    Ok(())
}

fn handle_supports(pre: impl Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args
        .get_one::<String>("renderer")
        .expect("Required argument");
    let supported = pre.supports_renderer(renderer);

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

pub struct ComparisonPreprocessor;

impl Preprocessor for ComparisonPreprocessor {
    fn name(&self) -> &str {
        "comparison-preprocessor"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        eprintln!("Running comparison preprocessor");
        book.for_each_mut(|item: &mut BookItem| {
            if let BookItem::Chapter(chapter) = item {
                let new = replace_examples(&chapter.content).unwrap();
                chapter.content.clear();
                chapter.content.push_str(&new);
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}

pub fn code_block_lang<'a>(event: &'a Event) -> Option<&'a str> {
    if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) = event {
        Some(lang.as_ref())
    } else {
        None
    }
}

fn replace_examples(text: &str) -> Result<String> {
    let mut parser = Parser::new_ext(text, Options::all());
    let mut cmark_acc = vec![];

    while let Some(event) = parser.next() {
        // If it's not PRQL, just push it and continue
        let Some(lang) = code_block_lang(&event) else {
            cmark_acc.push(event.to_owned());
            continue;
        };
        if !lang.starts_with("prql") {
            cmark_acc.push(event.to_owned());
            continue;
        }

        let Some(Event::Text(text)) = parser.next() else {
            bail!("Expected text within code block")
        };

        let prql = text.to_string();
        let options = prql_compiler::Options::default().no_signature();
        let result = compile(&prql, &options);

        match lang {
            "prql" | "prql_no_fmt" => cmark_acc.push(Event::Html(
                table_of_comparison(
                    &prql,
                    result
                        .map_err(|_| anyhow::anyhow!("Query raised an error:\n\n {prql}\n\n"))?
                        .as_str(),
                )
                .into(),
            )),
            "prql_error" => cmark_acc.push(Event::Html(
                table_of_error(
                    &prql,
                    result
                        .expect_err(
                            &format!(
                                "Query was labeled to raise an error, but succeeded.\n {prql}\n\n"
                            )
                            .to_string(),
                        )
                        .to_string()
                        .as_str(),
                )
                .into(),
            )),
            "prql_no_test" => {
                cmark_acc.push(Event::Html(table_of_prql_only(&prql).into()));
            }
            _ => {
                bail!("Unknown code block language: {}", lang)
            }
        };
        // Skip ending tag
        parser.next();
    }
    let mut buf = String::new();
    cmark(cmark_acc.into_iter(), &mut buf)?;

    Ok(buf)
}

fn table_of_comparison(prql: &str, sql: &str) -> String {
    format!(
        r#"
<div class="comparison">

<div>
<h4>PRQL</h4>

```prql
{prql}
```

</div>

<div>
<h4>SQL</h4>

```sql
{sql}
```

</div>

</div>
"#,
        prql = prql.trim(),
        sql = sql,
    )
    .trim_start()
    .to_string()
}

// Similar to `table_of_comparison`, but without a second column.
fn table_of_prql_only(prql: &str) -> String {
    format!(
        r#"
<div class="comparison">

<div>
<h4>PRQL</h4>

```prql
{prql}
```

</div>
</div>
"#,
        prql = prql.trim(),
    )
    .trim_start()
    .to_string()
}

// Exactly the same as `table_of_comparison`, but with a different title for the second column.
fn table_of_error(prql: &str, error: &str) -> String {
    format!(
        r#"
<div class="comparison">

<div>
<h4>PRQL</h4>

```prql
{prql}
```

</div>

<div>
<h4>Error</h4>

```
{error}
```

</div>

</div>
"#,
        prql = prql.trim(),
        error = error,
    )
    .trim_start()
    .to_string()
}

#[test]
fn test_replace_examples() -> Result<()> {
    use insta::assert_display_snapshot;

    let md = r###"
# PRQL Doc

```prql
from x
```

```python
import sys
```

```prql_error
this is an error
```
    "###;

    assert_display_snapshot!(replace_examples(md)?, @r###"
    # PRQL Doc

    <div class="comparison">

    <div>
    <h4>PRQL</h4>

    ```prql
    from x
    ```

    </div>

    <div>
    <h4>SQL</h4>

    ```sql
    SELECT
      *
    FROM
      x

    ```

    </div>

    </div>


    ````python
    import sys
    ````

    <div class="comparison">

    <div>
    <h4>PRQL</h4>

    ```prql
    this is an error
    ```

    </div>

    <div>
    <h4>Error</h4>

    ```
    Error:
       ╭─[:1:1]
       │
     1 │ this is an error
       │ ──┬─
       │   ╰─── Unknown name this
    ───╯

    ```

    </div>

    </div>
    "###);

    Ok(())
}

#[test]
fn test_table() -> Result<()> {
    use insta::assert_display_snapshot;
    let table = r###"
# Syntax

| a |
|---|
| c |


| a   |
|-----|
| \|  |

"###;

    assert_display_snapshot!(replace_examples(table)?, @r###"
    # Syntax

    |a|
    |-|
    |c|

    |a|
    |-|
    |\||
    "###);

    Ok(())
}
