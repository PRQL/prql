#![cfg(not(target_family = "wasm"))]
use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Result};
use globset::Glob;
use insta::assert_snapshot;
use itertools::Itertools;
use mdbook_prql::{code_block_lang_tags, LangTag};
use prqlc::sql::{Dialect, SupportLevel};
use prqlc::{pl_to_prql, pl_to_rq, prql_to_pl};
use pulldown_cmark::Tag;
use regex::Regex;
use strum::IntoEnumIterator;
use walkdir::WalkDir;

use super::compile;

/// This test:
/// - Extracts PRQL code blocks from the book
/// - Compiles them to SQL, comparing to a snapshot.
/// - We raise an error if they shouldn't pass or shouldn't fail.
/// - Insta raises an error if there's a snapshot diff.
///
/// This mirrors the process in [`replace_examples`], which inserts a comparison
/// table of SQL into the book, and so serves as a snapshot test of those
/// examples.
//
// We re-use the code (somewhat copy-paste) for the other compile tests below.
#[test]
fn test_prql_examples_compile() -> Result<()> {
    // Override the version so the examples work with the version defined in the
    // manifest.
    std::env::set_var("PRQL_VERSION_OVERRIDE", env!("CARGO_PKG_VERSION"));
    let examples = collect_book_examples()?;

    let mut errs = Vec::new();
    for Example { name, tags, prql } in examples {
        let result = compile(&prql);
        let should_succeed = !tags.contains(&LangTag::Error);

        match (should_succeed, result) {
            (true, Err(e)) => errs.push(format!(
                "
---- {name} ---- ERROR
Use `prql error` as the language label to assert an error compiling the PRQL.

-- Original PRQL --
```
{prql}
```

-- Error --
```
{e}
```
"
            )),

            (false, Ok(output)) => errs.push(format!(
                "
---- {name} ---- UNEXPECTED SUCCESS
Succeeded compiling, but example was marked as `error`.
Remove `error` as a language label to assert successfully compiling.

-- Original PRQL --
```
{prql}
```

-- Result --
```
{output}
```
"
            )),
            (_, result) => {
                assert_snapshot!(name, result.unwrap_or_else(|e| e.to_string()), &prql);
            }
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errs.join("\n")))
    }
}

#[test]
fn test_prql_examples_rq_serialize() -> Result<()> {
    for Example { tags, prql, .. } in collect_book_examples()? {
        // Don't assert that this fails, whether or not they compile to RQ is
        // undefined.
        if tags.contains(&LangTag::Error) {
            continue;
        }
        let rq = prql_to_pl(&prql).map(pl_to_rq)?;
        // Serialize
        serde_json::to_string(&rq).unwrap();
    }

    Ok(())
}

/// Test that the formatted result (the `Display` result) of each example can be
/// compiled.
//
// We previously snapshot all the queries. But that was a lot of output, for
// something we weren't yet looking at.
//
// The ideal would be to auto-format the examples themselves, likely during the
// compilation. For that to provide a good output, we need to implement a proper
// autoformatter.
#[test]
fn test_prql_examples_display_then_compile() -> Result<()> {
    let examples = collect_book_examples()?;

    let mut errs = Vec::new();
    for Example { name, tags, prql } in examples {
        let result = prql_to_pl(&prql)
            .and_then(|x| pl_to_prql(&x))
            .and_then(|x| compile(&x));

        let should_succeed = !tags.contains(&LangTag::NoFmt);

        match (should_succeed, result) {
            (true, Err(e)) => errs.push(format!(
                "
---- {name} ---- ERROR formatting & compiling
Use `prql no-fmt` as the language label to assert an error from formatting & compiling.

-- Original PRQL --

```
{prql}
```
-- Error --
```
{e}
```
"
            )),

            (false, Ok(output)) => errs.push(format!(
                "
---- {name} ---- UNEXPECTED SUCCESS after formatting
Succeeded at formatting and then compiling the prql, but example was marked as `no-fmt`.
Remove `no-fmt` as a language label to assert successfully compiling the formatted result.

-- Original PRQL --
```
{prql}
```
-- Result --
```
{output}
```
"
            )),
            _ => {}
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errs.join("")))
    }
}

/// The date & time format specifier table in `reference/stdlib/date.md` is
/// introduced as "the list of the specifiers currently supported", so assert
/// that each row does compile — otherwise the table silently drifts from the
/// dialect implementations in `sql/dialect.rs`.
///
/// We check against Postgres, matching the other examples on that page; which
/// specifiers are supported varies by dialect.
#[test]
fn test_date_format_specifiers_compile() -> Result<()> {
    let text = fs::read_to_string("./src/reference/stdlib/date.md")?;
    // The first cell of each row of the specifier table, e.g. ``| `%Y` |``.
    let specifier_cell = Regex::new(r"(?m)^\| `(%[^`]+)` *\|")?;

    let specifiers: Vec<&str> = specifier_cell
        .captures_iter(&text)
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    // Guard against the test passing vacuously if the table's formatting
    // changes and the regex stops matching.
    if specifiers.len() < 20 {
        bail!(
            "Expected to find the date specifier table; only matched {} rows",
            specifiers.len()
        );
    }

    let mut errs = Vec::new();
    for specifier in specifiers {
        let prql = format!(
            r#"prql target:sql.postgres
from tbl
select {{ a = (tbl.col | date.to_text "{specifier}") }}
"#
        );
        if let Err(e) = compile(&prql) {
            errs.push(format!(
                "`{specifier}` is documented as supported, but fails to compile:\n{e}"
            ));
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(errs.join("\n")))
    }
}

/// The "Supported" / "Unsupported" dialect lists in `project/target.md` are a
/// hand-maintained copy of `Dialect::support_level` in `sql/dialect.rs`, so
/// assert the two agree — otherwise a dialect can ship without ever being
/// documented, as `sql.redshift` did.
#[test]
fn test_target_dialects_documented() -> Result<()> {
    let text = fs::read_to_string("./src/project/target.md")?;

    // Each list is a run of ``- `sql.<name>` `` bullets under its heading;
    // bullets may carry trailing prose, and the section ends at the next
    // heading.
    let section = |heading: &str| -> Result<Vec<String>> {
        let body = text
            .split_once(heading)
            .map(|(_, rest)| rest.split(['#']).next().unwrap_or(rest))
            .ok_or_else(|| anyhow!("`{heading}` heading not found in target.md"))?;
        let bullet = Regex::new(r"(?m)^- `sql\.([a-z]+)`")?;
        Ok(bullet
            .captures_iter(body)
            .map(|c| c[1].to_string())
            .sorted()
            .collect())
    };

    let documented_supported = section("### Supported")?;
    let documented_unsupported = section("### Unsupported")?;

    let actual = |level: SupportLevel| -> Vec<String> {
        Dialect::iter()
            .filter(|d| {
                std::mem::discriminant(&d.support_level()) == std::mem::discriminant(&level)
            })
            .map(|d| d.to_string())
            .sorted()
            .collect()
    };

    similar_asserts::assert_eq!(
        documented: documented_supported,
        actual: actual(SupportLevel::Supported),
    );
    similar_asserts::assert_eq!(
        documented: documented_unsupported,
        actual: actual(SupportLevel::Unsupported),
    );
    Ok(())
}

struct Example {
    /// Name contains the file, the heading, and the index of the example.
    name: String,
    tags: Vec<LangTag>,
    /// The PRQL text
    prql: String,
}

/// Collect all the PRQL examples in the book, as [Example]s.
/// Excludes any with a `no-eval` tag.
fn collect_book_examples() -> Result<Vec<Example>> {
    use pulldown_cmark::{Event, Parser};
    let glob = Glob::new("**/*.md")?.compile_matcher();
    Ok(WalkDir::new(Path::new("./src/"))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .flat_map(|dir_entry| {
            let text = fs::read_to_string(dir_entry.path())?;
            // TODO: Still slightly duplicative logic here and in
            // [lib.rs/replace_examples], but not sure how to avoid it.
            //
            let mut parser = Parser::new(&text);
            let mut prql_blocks: Vec<Example> = vec![];
            // Keep track of the latest heading, so snapshots can have the
            // section they're in. This makes them easier to find and means
            // adding one example at the top of the book doesn't cause a huge
            // diff in the snapshots of that file's examples..
            let mut latest_heading = "".to_string();
            let file_name = &dir_entry
                .path()
                .strip_prefix("./src/")?
                .to_str()
                .unwrap()
                .trim_end_matches(".md");

            // Iterate through the markdown file, getting examples.
            while let Some(event) = parser.next() {
                if let Event::Start(Tag::Heading { .. }) = event.clone() {
                    if let Some(Event::Text(pulldown_cmark::CowStr::Borrowed(heading))) =
                        parser.next()
                    {
                        // We clear and then push because just setting
                        // `latest_heading` leads to lifetime issues.
                        latest_heading = heading
                            .chars()
                            .filter(|&c| c.is_ascii_alphanumeric() || c == '-' || c == ' ')
                            .collect();
                    }
                }
                let Some(tags) = code_block_lang_tags(&event) else {
                    continue;
                };

                if tags.contains(&LangTag::Prql) && !tags.contains(&LangTag::NoEval) {
                    let mut prql = String::new();
                    while let Some(Event::Text(line)) = parser.next() {
                        prql.push_str(line.to_string().as_str());
                    }
                    if prql.is_empty() {
                        bail!("Expected text in PRQL code block");
                    }
                    let heading = latest_heading.replace(' ', "-").to_ascii_lowercase();
                    // Only add the heading if it's different from the file name.
                    let name = if !file_name.ends_with(&heading) {
                        format!("{file_name}/{heading}")
                    } else {
                        file_name.to_string()
                    };
                    prql_blocks.push(Example { name, tags, prql });
                }
            }
            Ok(prql_blocks)
        })
        .flatten()
        // Add an index suffix to each path's examples (so we group by the path).
        .chunk_by(|e| e.name.clone())
        .into_iter()
        .flat_map(|(path, blocks)| {
            blocks.into_iter().enumerate().map(move |(i, e)| Example {
                name: format!("{path}/{i}"),
                ..e
            })
        })
        .collect())
}
