#![cfg(not(target_family = "wasm"))]
use anyhow::{bail, Result};
use globset::Glob;
use insta::assert_snapshot;
use mdbook_prql::{code_block_lang_tags, LangTag};
use prql_compiler::*;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// This test:
/// - Extracts PRQL code blocks from the book
/// - Compiles them to SQL, comparing to a snapshot.
/// - We raise an error if they shouldn't pass or shouldn't fail.
/// - Insta raises an error if there's a snapshot diff.
///
/// This mirrors the process in [replace_examples], which inserts a comparison
/// table of SQL into the book, and so serves as a snapshot test of those
/// examples.
//
// We re-use the code (somewhat copy-paste) for the other compile tests below.
#[test]
fn test_prql_examples_compile() -> Result<()> {
    collect_book_examples()?
        .iter()
        .try_for_each(|Example { name, tags, prql }| {
            let result = compile(prql, &Options::default().no_signature());
            let should_succeed = !tags.contains(&LangTag::Error);

            match (should_succeed, result) {
                (true, Err(e)) => bail!(
                    "
Failed compiling {name:?}
Use `prql error` as the language label to assert an error compiling the PRQL.

The original PRQL:

{prql}

And the error:

{e}

"
                ),

                (false, Ok(output)) => bail!(
                    "
Succeeded compiling {name:?}, but example was marked as `error`.
Remove `error` as a language label to assert successfully compiling.

The original PRQL:

{prql}

And the result:

{output}

"
                ),
                (_, result) => {
                    assert_snapshot!(
                        name.to_string(),
                        result.unwrap_or_else(|e| e.to_string()),
                        prql
                    );
                    Ok(())
                }
            }
        })
}

#[test]
fn test_prql_examples_rq_serialize() -> Result<(), ErrorMessages> {
    for Example { tags, prql, .. } in collect_book_examples()? {
        // Don't assert that this fails, whether or not they compile to RQ is
        // undefined.
        if tags.contains(&LangTag::Error) {
            continue;
        }
        let rq = prql_to_pl(&prql).map(pl_to_rq)?;
        // Serialize to YAML
        assert!(serde_yaml::to_string(&rq).is_ok());
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
    collect_book_examples()?
        .iter()
        .try_for_each(|Example { name, tags, prql }| {
            let result = prql_to_pl(prql)
                .and_then(pl_to_prql)
                .and_then(|formatted| compile(&formatted, &Options::default()));
            let should_succeed = !tags.contains(&LangTag::NoFmt);

            match (should_succeed, result) {
                (true, Err(e)) => bail!(
                    "
Failed compiling the formatted result of {name:?}
Use `prql no-fmt` as the language label to assert an error from compiling the formatted result.

The original PRQL:

{prql}

And the error:

{e}

"
                ),

                (false, Ok(output)) => bail!(
                    "
Succeeded at compiling the formatted result of {name:?}, but example was marked as `no-fmt`.
Remove `no-fmt` as a language label to assert successfully compiling the formatted resullt.

The original PRQL:

{prql}

And the result:

{output}

"
                ),
                _ => Ok(()),
            }
        })
}

struct Example {
    name: String,
    tags: Vec<LangTag>,
    prql: String,
}

/// Collect all the PRQL examples in the book, as [Example]s.
/// Excludes any with a `no-eval` tag.
fn collect_book_examples() -> Result<Vec<Example>> {
    use pulldown_cmark::{Event, Parser};
    let glob = Glob::new("**/*.md")?.compile_matcher();
    let examples_in_book: Vec<Example> = WalkDir::new(Path::new("./src/"))
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

                if lang_tags.contains(&LangTag::Prql) && !lang_tags.contains(&LangTag::NoEval) {
                    let mut prql_text = String::new();
                    while let Some(Event::Text(line)) = parser.next() {
                        prql_text.push_str(line.to_string().as_str());
                    }
                    if prql_text.is_empty() {
                        bail!("Expected text after PRQL code block");
                    }
                    prql_blocks.push((lang_tags, prql_text));
                }
            }
            let file_name = &dir_entry
                .path()
                .strip_prefix("./src/")?
                .to_str()
                .unwrap()
                .trim_end_matches(".md");
            Ok(prql_blocks
                .into_iter()
                .enumerate()
                .map(|(i, (tags, prql))| Example {
                    name: format!("{file_name}-{i}"),
                    tags,
                    prql,
                })
                .collect::<Vec<Example>>())
        })
        .flatten()
        .collect();

    Ok(examples_in_book)
}
