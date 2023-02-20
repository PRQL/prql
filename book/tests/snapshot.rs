#![cfg(not(target_family = "wasm"))]
/// This test:
/// - Extracts PRQL code blocks into files in the `examples` path
/// - Converts them to SQL using insta, raising an error if there's a diff.
/// - Replaces the PRQL code block with a comparison table.
///
/// We also use this test to run tests on our Display trait output, currently as
/// another set of snapshots (more comments inline).
//
// Overall, this is overengineered — it's complicated and took a long time to
// write. The intention is good — have a version of the SQL that's committed
// into the repo, and join our tests with our docs. But it feels like overly
// custom code for quite a general problem, even if our preferences are slightly
// different from the general case.
//
// Possibly we should be using something like pandoc /
// https://github.com/gpoore/codebraid / which would run the transformation for
// us. They introduce a bunch of non-rust dependencies, which is not ideal, but
// passable. They don't let us customize our formatting (e.g. in a table).
//
use anyhow::{bail, Error, Result};
use globset::Glob;
use insta::{assert_snapshot, glob};
use log::warn;
use prql_compiler::*;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};
use walkdir::WalkDir;

#[test]
fn test_examples() -> Result<()> {
    // Note that on windows, markdown is read differently, and so
    // writing on Windows. ref https://github.com/PRQL/prql/issues/356
    #[cfg(not(target_family = "windows"))]
    write_prql_snapshots()?;
    test_prql_examples();

    Ok(())
}

const ROOT_EXAMPLES_PATH: &str = "tests/prql";

/// Collect all the PRQL examples in the book, as a map of <Path, PRQL>.
#[cfg(not(target_family = "windows"))]
fn collect_book_examples() -> Result<HashMap<PathBuf, String>> {
    let glob = Glob::new("**/*.md")?.compile_matcher();
    let examples_in_book: HashMap<PathBuf, String> = WalkDir::new(Path::new("./src/"))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .flat_map(|dir_entry| {
            let text = fs::read_to_string(dir_entry.path())?;
            let mut parser = Parser::new(&text);
            let mut prql_blocks = vec![];
            while let Some(event) = parser.next() {
                match event.clone() {
                    // At the start of a PRQL code block, push the _next_ item.
                    // Note that on windows, we only get the next _line_, and so
                    // we exclude the writing in windows below;
                    // https://github.com/PRQL/prql/issues/356
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                        if lang == "prql".into() =>
                    {
                        if let Some(Event::Text(text)) = parser.next() {
                            prql_blocks.push(text);
                        } else {
                            bail!("Expected text after PRQL code block");
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
#[cfg(not(target_family = "windows"))]
fn collect_snapshot_examples() -> Result<HashMap<PathBuf, String>> {
    use itertools::Itertools;
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
fn collect_snapshot_examples() -> Result<HashMap<PathBuf, String>> {
    collect_snapshot_examples()
}

/// Extract reference examples from the PRQL docs and write them to the
/// `tests/prql` path, one in each file.
// We could alternatively have used something like
// https://github.com/earldouglas/codedown, but it's not much code, and it
// requires no dependencies.
//
// We allow dead_code because of the window issue described above. (Can we allow
// it only for windows?)
#[allow(dead_code)]
fn write_prql_snapshots() -> Result<()> {
    // If we have to modify any files, raise an error at the end, so it fails in CI.
    let mut is_snapshots_updated = false;

    let mut existing_snapshots: HashMap<_, _> = collect_snapshot_examples()?;
    // Write any new snapshots, or update any that have changed. =
    collect_book_examples()?
        .iter()
        .try_for_each(|(prql_path, example)| {
            if existing_snapshots
                .remove(prql_path)
                .map(|existing| existing != *example)
                .unwrap_or(true)
            {
                is_snapshots_updated = true;
                fs::create_dir_all(Path::new(prql_path).parent().unwrap())?;
                fs::write(prql_path, example)?;
            }

            Ok::<(), anyhow::Error>(())
        })?;

    // If there are any files left in `existing_snapshots`, we remove them, since
    // they don't reference anything.
    existing_snapshots.iter().for_each(|(path, _)| {
        trash::delete(path).unwrap_or_else(|e| {
            warn!("Failed to delete unreferenced example: {}", e);
        })
    });

    if is_snapshots_updated {
        bail!("Some book snapshots were not consistent with the queries in the book. The snapshots have now been updated. Subsequent runs should pass.");
    }
    Ok(())
}

/// Snapshot the SQL output of each example.
fn test_prql_examples() {
    glob!("prql/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();

        if prql.contains("skip_test") {
            return;
        }

        let opts = Options::default().no_signature();
        let sql = compile(&prql, opts).unwrap_or_else(|e| format!("{prql}\n\n{e}"));
        // `glob!` gives us the file path in the test name anyway, so we pass an
        // empty name. We pass `&prql` so the prql is in the snapshot (albeit in
        // a single line, and, in the rare case that the SQL doesn't change, the
        // PRQL only updates on running cargo insta with `--force-update-snapshots`).
        assert_snapshot!("", &sql, &prql);
    });
}

/// Snapshot the display trait output of each example.
//
// TODO: this involves writing out almost the same PRQL again — instead we could
// compare the output of Display to the auto-formatted source. But we need an
// autoformatter for that (unless we want to raise on any non-matching input,
// which seems very strict)
#[test]
fn test_display() -> Result<(), ErrorMessages> {
    use prql_compiler::downcast;
    collect_book_examples()
        .map_err(downcast)?
        .iter()
        .try_for_each(|(path, example)| {
            assert_snapshot!(
                path.to_string_lossy().to_string(),
                prql_to_pl(example).and_then(pl_to_prql)?,
                example
            );
            Ok::<(), ErrorMessages>(())
        })?;

    Ok(())
}
