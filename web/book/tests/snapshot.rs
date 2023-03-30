#![cfg(not(target_family = "wasm"))]
use anyhow::{bail, Result};
use globset::Glob;
use insta::assert_snapshot;
use mdbook_prql::code_block_lang_tags;
use prql_compiler::*;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};
use walkdir::WalkDir;

#[test]
/// This test:
/// - Extracts PRQL code blocks from the book
/// - Compiles them to SQL, comparing to a snapshot. Insta raises an error if
///   there's a diff.
///
/// This mirrors the process in [replace_examples], which inserts a
/// comparison table of SQL into the book, and so serves as a snapshot test of
/// those examples.
/// Snapshot the SQL output of each example.
fn test_prql_examples() {
    let opts = Options::default().no_signature();
    let examples = collect_book_examples().unwrap();

    for (path, prql) in examples {
        // Whether it's a success or a failure, get the string.
        let sql = compile(&prql, &opts).unwrap_or_else(|e| e.to_string());
        assert_snapshot!(path.to_str().unwrap(), &sql, &prql);
    }
}

const ROOT_EXAMPLES_PATH: &str = "tests/prql";

/// Collect all the PRQL examples in the book, as a map of <Path, PRQL>.
fn collect_book_examples() -> Result<HashMap<PathBuf, String>> {
    // TODO: instead of returning Strings with embedded tags (e.g. `# error`),
    // we could instead return a struct with a `prql` field and a struct of its
    // metadata. That would make `test_display` work by matching on the metadata
    // rather than re-parsing the string.
    use pulldown_cmark::{Event, Parser};
    let glob = Glob::new("**/*.md")?.compile_matcher();
    let examples_in_book: HashMap<PathBuf, String> = WalkDir::new(Path::new("./src/"))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .flat_map(|dir_entry| {
            let text = fs::read_to_string(dir_entry.path())?;
            // TODO: Still slightly duplicative logic here and in
            // [lib.rs/replace_examples], but not sure how to avoid it.
            //
            let mut parser = Parser::new(&text);
            let mut prql_blocks = vec![];
            while let Some(event) = parser.next() {
                let Some(lang_tags) = code_block_lang_tags(&event) else {
                    continue
                };

                if lang_tags.contains(&"prql".to_string()) {
                    let mut prql_text = String::new();
                    while let Some(Event::Text(line)) = parser.next() {
                        prql_text.push_str(line.to_string().as_str());
                    }
                    if prql_text.is_empty() {
                        bail!("Expected text after PRQL code block");
                    }
                    if lang_tags.contains(&"error".to_string()) {
                        prql_blocks.push(format!("# error\n\n{prql_text}"));
                    } else if lang_tags.contains(&"no-fmt".to_string()) {
                        prql_blocks.push(format!("# no-fmt\n\n{prql_text}"));
                    } else {
                        prql_blocks.push(prql_text.to_string());
                    }
                }
            }
            let snapshot_prefix = &dir_entry
                .path()
                .strip_prefix("./src/")?
                .to_str()
                .unwrap()
                .trim_end_matches(".md");
            Ok(prql_blocks
                .iter()
                .enumerate()
                .map(|(i, example)| {
                    (
                        Path::new(&format!("{ROOT_EXAMPLES_PATH}/{snapshot_prefix}-{i}.prql"))
                            .to_path_buf(),
                        example.to_string(),
                    )
                })
                .collect::<HashMap<_, _>>())
        })
        .flatten()
        .collect();

    Ok(examples_in_book)
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
fn test_display() -> Result<(), ErrorMessages> {
    collect_book_examples()?
        .iter()
        .try_for_each(|(path, prql)| {
            if prql.contains("# error") || prql.contains("# no-fmt") {
                return Ok(());
            }
            prql_to_pl(prql)
                .and_then(pl_to_prql)
                .and_then(|formatted| compile(&formatted, &Options::default()))
                .unwrap_or_else(|_| {
                    panic!(
                        "
Failed compiling the formatted result of {path:?}
To skip this test for an example, use `prql,no-fmt` as the language label.

The original PRQL was:

{prql}

",
                        path = path.canonicalize().unwrap(),
                        prql = prql
                    )
                });

            Ok::<(), ErrorMessages>(())
        })?;

    Ok(())
}

#[test]
fn test_rq_serialize() -> Result<(), ErrorMessages> {
    for (_, prql) in collect_book_examples()? {
        if prql.contains("# error") {
            continue;
        }
        let rq = prql_to_pl(&prql).map(pl_to_rq)?;
        // Serialize to YAML
        assert!(serde_yaml::to_string(&rq).is_ok());
    }

    Ok(())
}
