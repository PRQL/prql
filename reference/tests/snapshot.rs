/// This test:
/// - Extracts PRQL code blocks into the `examples` path.
/// - Converts them to SQL using insta, raising an error if there's a diff.
/// - Replaces the PRQL code block with a comparison table.
//
// Overall, this is bad quality code that I shouldn't have written. It also
// took a long time to write. The intention was reasonable — have a version of
// the SQL that's committed into the repo, and join our tests with our docs.
//
// We don't use a book preprocessor, because we want to the results committed
// into the repository, so we can see if anything changes (I think this
// dimension is quite important.)
//
// Possibly we should be using something like pandoc /
// https://github.com/gpoore/codebraid / which would run the transformation for
// us. They introduce a bunch of non-rust dependencies, which is not ideal, but
// passable. They don't let us customize our formatting (e.g. in a table).
//
// Overall, this feels like overly custom code for quite a general problem, even
// if our preferences are slightly different.
use anyhow::{bail, Result};
use globset::Glob;
use insta::{assert_snapshot, glob};
use prql::*;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[test]
fn run_examples() -> Result<()> {
    write_reference_examples()?;
    run_reference_examples()?;

    Ok(())
}

/// Extract reference examples from the PRQL docs and write them to the
/// `tests/examples` path, one in each file.
// We could alternatively have used something like
// https://github.com/earldouglas/codedown, but it's not much code, and it
// requires no dependencies.
fn write_reference_examples() -> Result<()> {
    let glob = Glob::new("**/*.md")?.compile_matcher();

    WalkDir::new(Path::new("./src/"))
        .into_iter()
        .flatten()
        .filter(|x| glob.is_match(x.path()))
        .try_for_each(|dir_entry| {
            let text = fs::read_to_string(dir_entry.path())?;
            let mut parser = Parser::new(&text);
            let mut examples = vec![];
            while let Some(event) = parser.next() {
                match event.clone() {
                    // At the start of a PRQL code block, push the _next_ item.
                    Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang)))
                        if lang == "prql".into() =>
                    {
                        if let Some(Event::Text(text)) = parser.next() {
                            examples.push(text);
                        } else {
                            bail!("Expected text after PRQL code block");
                        }
                    }
                    _ => {}
                }
            }

            // Write each one to a new file.
            examples.iter().enumerate().try_for_each(|(i, example)| {
                let file_relative = &dir_entry
                    .path()
                    .strip_prefix("./src/")?
                    .to_str()
                    .unwrap()
                    .trim_end_matches(".md");
                let prql_path = format!("tests/examples/{file_relative}-{i}.prql");

                fs::create_dir_all(Path::new(&prql_path).parent().unwrap())?;
                fs::write(prql_path, example.to_string())?;

                Ok::<(), anyhow::Error>(())
            })?;
            Ok(())
        })?;

    Ok(())
}

/// Snapshot the output of each example.
fn run_reference_examples() -> Result<()> {
    glob!("examples/**/*.prql", |path| {
        let prql = fs::read_to_string(path).unwrap();

        if prql.contains("skip_test") {
            return;
        }

        let sql = compile(&prql).unwrap_or_else(|e| format!("Failed to compile `{prql}`; {e}"));
        assert_snapshot!(sql);
    });
    Ok(())
}
