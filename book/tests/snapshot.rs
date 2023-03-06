#![cfg(not(target_family = "wasm"))]
//
// Thoughts on the overall code:
//
// Overall, this is overengineered — it's complicated and took a long time to
// write. The intention is good — have a version of the SQL that's committed
// into the repo, and join our tests with our docs. But it feels like overly
// custom code for quite a general problem, even if our preferences are slightly
// different from the general case.
//
// Having an API for being able to read snapshots
// (https://github.com/mitsuhiko/insta/issues/353) would significantly reduce the need for
// custom code;
//
// Possibly we should be using something like pandoc /
// https://github.com/gpoore/codebraid / which would run the transformation for
// us. They introduce a bunch of non-rust dependencies, which is not ideal, but
// passable. They don't let us customize our formatting (e.g. in a table).
//
use anyhow::{bail, Error, Result};
use globset::Glob;
use insta::{assert_snapshot, glob};
use itertools::Itertools;
use log::warn;
use prql_compiler::*;
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};
use walkdir::WalkDir;

#[test]
/// This test:
/// - Extracts PRQL code blocks into files in the `examples` path, skipping
///   where the matching example is already present.
/// - Compiles them to SQL, comparing to a snapshot. Insta raises an error if
///   there's a diff.
///
/// Then, when the book is built, the PRQL code block in the book is replaced
/// with a comparison table.
fn test_examples() -> Result<()> {
    // Note that on Windows, markdown is read differently, and so we don't write
    // on Windows (we write from the same place we read as a workaround). ref
    // https://github.com/PRQL/prql/issues/356

    write_prql_examples(collect_book_examples()?)?;
    test_prql_examples();

    Ok(())
}

const ROOT_EXAMPLES_PATH: &str = "tests/prql";

/// Collect all the PRQL examples in the book, as a map of <Path, PRQL>.
#[cfg(not(target_family = "windows"))]
fn collect_book_examples() -> Result<HashMap<PathBuf, String>> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
    let glob = Glob::new("**/*.md")?.compile_matcher();
    let examples_in_book: HashMap<PathBuf, String> = WalkDir::new(Path::new("./src/"))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .flat_map(|dir_entry| {
            let text = fs::read_to_string(dir_entry.path())?;
            // TODO: Duplicative logic here and in [lib.rs/replace_examples];
            // could we unify?
            //
            // Could we have a function that takes text and returns a
            // Vec<prql_string, result, expected>, where expected is whether it
            // should succeed or fail?
            let mut parser = Parser::new(&text);
            let mut prql_blocks = vec![];
            while let Some(event) = parser.next() {
                match event.clone() {
                    // At the start of a PRQL code block, push the _next_ item.
                    // Note that on windows, we only get the next _line_, and so
                    // this is disabled on windows.
                    // https://github.com/PRQL/prql/issues/356
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                        if lang.starts_with("prql") =>
                    {
                        let Some(Event::Text(text)) = parser.next() else {
                            bail!("Expected text after PRQL code block")
                        };
                        if lang == "prql".into() {
                            prql_blocks.push(text.to_string());
                        } else if lang == "prql_error".into() {
                            prql_blocks.push(format!("# Error expected\n\n{text}"));
                        } else if lang == "prql_no_fmt".into() {
                            prql_blocks.push(format!("# Can't yet format & compile\n\n{text}"));
                        }
                    }
                    _ => {}
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

/// Collect examples which we've already written to disk, as a map of <Path, PRQL>.
fn collect_snapshot_examples() -> Result<HashMap<PathBuf, String>> {
    let glob = Glob::new("**/*.prql")?.compile_matcher();
    let existing_examples = WalkDir::new(Path::new(ROOT_EXAMPLES_PATH))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .map(|x| Ok::<_, Error>((x.clone().into_path(), fs::read_to_string(x.path())?)))
        .try_collect()?;

    Ok(existing_examples)
}

// On Windows, we grab them from the written files, because of the markdown issue.
#[cfg(target_family = "windows")]
fn collect_book_examples() -> Result<HashMap<PathBuf, String>> {
    collect_snapshot_examples()
}

/// Write the passed examples as snapshots to the `tests/prql` path, one in each file.
// We could alternatively have used something like
// https://github.com/earldouglas/codedown, but it's not much code, and it
// requires no dependencies.
fn write_prql_examples(examples: HashMap<PathBuf, String>) -> Result<()> {
    // If we have to modify any files, raise an error at the end, so it fails in CI.
    let mut snapshots_updated = vec![];

    let mut existing_snapshots: HashMap<_, _> = collect_snapshot_examples()?;
    // Write any new snapshots, or update any that have changed
    examples.iter().try_for_each(|(prql_path, example)| {
        if existing_snapshots
            .remove(prql_path)
            .map(|existing| existing != *example)
            .unwrap_or(true)
        {
            snapshots_updated.push(prql_path);
            fs::create_dir_all(Path::new(prql_path).parent().unwrap())?;
            fs::write(prql_path, example)?;
        }

        Ok::<(), anyhow::Error>(())
    })?;

    // If there are any files left in `existing_snapshots`, we remove them,
    // since they don't reference anything (like
    // `--delete-unreferenced-snapshots` in insta).
    existing_snapshots.iter().for_each(|(path, _)| {
        trash::delete(path).unwrap_or_else(|e| {
            warn!("Failed to delete unreferenced example: {}", e);
        })
    });

    if !snapshots_updated.is_empty() {
        let snapshots_updated = snapshots_updated
            .iter()
            .map(|x| format!("  - {}", x.to_str().unwrap()))
            .join("\n");
        bail!(format!(
            r###"
Some book snapshots were not consistent with the queries in the book:

{snapshots_updated}

The snapshots have now been updated. Subsequent runs of this test should now pass."###
        ));
    }
    Ok(())
}

/// Snapshot the SQL output of each example.
fn test_prql_examples() {
    let opts = Options::default().no_signature();
    glob!("prql/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();

        if prql.contains("skip_test") {
            return;
        }

        // Whether it's a success or a failure, get the string.
        let sql = compile(&prql, &opts).unwrap_or_else(|e| e.to_string());
        // `glob!` gives us the file path in the test name anyway, so we pass an
        // empty name. We pass `&prql` so the prql is in the snapshot (albeit in
        // a single line, and, in the rare case that the SQL doesn't change, the
        // PRQL only updates on running cargo insta with `--force-update-snapshots`).
        assert_snapshot!("", &sql, &prql);
    });
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
            if prql.contains("# Error expected") || prql.contains("# Can't yet format & compile") {
                return Ok(());
            }
            prql_to_pl(prql)
                .and_then(pl_to_prql)
                .and_then(|formatted| compile(&formatted, &Options::default()))
                .unwrap_or_else(|_| {
                    panic!(
                        "
Failed compiling the formatted result of {path:?}
To skip this test for an example, use `prql_no_fmt` as the language label.

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
