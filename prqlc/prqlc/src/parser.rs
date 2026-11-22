use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

use itertools::Itertools;

use crate::debug;
use crate::lr;
use crate::pr;
use crate::{Error, Errors, Result, SourceTree, WithErrorInfo};

pub fn parse(file_tree: &SourceTree) -> Result<pr::ModuleDef, Errors> {
    // register a new stage of the compiler
    // (here should register lexer stage first, but that all happens in a single call to prqlc_parser)
    debug::log_entry(|| debug::DebugEntryKind::ReprPrql(file_tree.clone()));
    debug::log_stage(debug::Stage::Parsing);

    let source_files = linearize_tree(file_tree)?;

    // reverse the id->file_path map
    let ids: HashMap<_, _> = file_tree
        .source_ids
        .iter()
        .map(|(a, b)| (b.as_path(), a))
        .collect();

    // init the root module def
    let mut root = pr::ModuleDef {
        name: "Project".to_string(),
        stmts: Vec::new(),
    };

    // parse and insert into the root
    let mut errors = Vec::new();
    for source_file in source_files {
        let id = ids
            .get(&source_file.file_path)
            .map(|x| **x)
            .expect("source tree has malformed ids");

        match parse_source(source_file.content, id) {
            Ok(stmts) => {
                insert_stmts_at_path(&mut root, source_file.module_path, stmts);
            }
            Err(errs) => errors.extend(errs),
        }
    }
    if errors.is_empty() {
        debug::log_entry(|| debug::DebugEntryKind::ReprPr(root.clone()));
        Ok(root)
    } else {
        Err(Errors(errors))
    }
}

/// Build PR AST from a PRQL query string.
// We have this function in `prqlc` rather than in `prqlc-parser` crate since
// our logging is in `prqlc` and we want to log the LR. (We could split the logging
// out into a separate crate, but it has dependencies on `prqlc` internals and
// would be an effort)
pub(crate) fn parse_source(source: &str, source_id: u16) -> Result<Vec<pr::Stmt>, Vec<Error>> {
    let (tokens, mut errors) = prqlc_parser::lexer::lex_source_recovery(source, source_id);

    let ast = if let Some(tokens) = tokens {
        debug::log_entry(|| debug::DebugEntryKind::ReprLr(lr::Tokens(tokens.clone())));

        let (ast, parse_errors) = prqlc_parser::parser::parse_lr_to_pr(source_id, tokens);
        errors.extend(parse_errors);
        ast
    } else {
        None
    };

    if errors.is_empty() {
        Ok(ast.unwrap_or_default())
    } else {
        Err(errors)
    }
}

struct SourceFile<'a> {
    file_path: &'a Path,
    module_path: Vec<String>,
    content: &'a str,
}

fn linearize_tree(tree: &SourceTree) -> Result<Vec<SourceFile<'_>>> {
    // find root
    let root_path;

    if tree.sources.len() == 1 {
        // if there is only one file, use that as the root
        root_path = tree.sources.keys().next().unwrap();
    } else if let Some(root) = tree.sources.get_key_value(&PathBuf::from("")) {
        // if there is an empty path, that's the root
        root_path = root.0;
    } else if let Some(root) = find_uppercase_root(tree)? {
        root_path = root;
    } else {
        if tree.sources.is_empty() {
            // TODO: should we allow non `.prql` files? We could require `.prql`
            // for modules but then allow any file if a single file is passed
            // (python allows this, for example)
            return Err(Error::new_simple(
                "No `.prql` files found in the source tree",
            ));
        }

        let file_names = tree
            .sources
            .keys()
            .map(|p| format!(" - {}", p.to_str().unwrap_or_default()))
            .sorted()
            .join("\n");

        return Err(Error::new_simple(format!(
            "Cannot find the root module within the following files:\n{file_names}"
        ))
        .push_hint("add a file that starts with uppercase letter to the root directory")
        .with_code("E0002"));
    }

    let mut sources: Vec<_> = Vec::with_capacity(tree.sources.len());

    // prepare paths
    for (path, source) in &tree.sources {
        if path == root_path {
            continue;
        }

        let module_path = os_path_to_prql_path(path)?;

        sources.push(SourceFile {
            file_path: path,
            module_path,
            content: source,
        });
    }

    // sort to make this deterministic
    sources.sort_by(|a, b| a.module_path.cmp(&b.module_path));

    // add root
    let root_content = tree.sources.get(root_path).unwrap();
    sources.push(SourceFile {
        file_path: root_path,
        module_path: Vec::new(),
        content: root_content,
    });

    Ok(sources)
}

fn insert_stmts_at_path(module: &mut pr::ModuleDef, mut path: Vec<String>, stmts: Vec<pr::Stmt>) {
    if path.is_empty() {
        module.stmts.extend(stmts);
        return;
    }

    let step = path.remove(0);

    // find submodule def
    let submodule = module.stmts.iter_mut().find(|x| is_mod_def_for(x, &step));
    let submodule = if let Some(sm) = submodule {
        sm
    } else {
        // insert new module def
        let new_stmt = pr::Stmt::new(pr::StmtKind::ModuleDef(pr::ModuleDef {
            name: step,
            stmts: Vec::new(),
        }));
        module.stmts.push(new_stmt);
        module.stmts.last_mut().unwrap()
    };
    let submodule = submodule.kind.as_module_def_mut().unwrap();

    insert_stmts_at_path(submodule, path, stmts);
}

pub(crate) fn is_mod_def_for(stmt: &pr::Stmt, name: &str) -> bool {
    stmt.kind.as_module_def().is_some_and(|x| x.name == name)
}

/// Finds the root module among files in the root directory whose name starts
/// with an uppercase letter. Errors when more than one file qualifies, rather
/// than picking one by hash map iteration order.
fn find_uppercase_root(tree: &SourceTree) -> Result<Option<&PathBuf>> {
    let candidates: Vec<&PathBuf> = tree
        .sources
        .keys()
        .filter(|p| is_uppercase_root_file(p))
        .sorted()
        .collect();

    match candidates.as_slice() {
        [] => Ok(None),
        [root] => Ok(Some(root)),
        _ => {
            let file_names = candidates
                .iter()
                .map(|p| format!(" - {}", p.to_str().unwrap_or_default()))
                .join("\n");
            Err(Error::new_simple(format!(
                "Found more than one root module candidate:\n{file_names}"
            ))
            .push_hint("keep only one file starting with an uppercase letter in the root directory")
            .with_code("E0002"))
        }
    }
}

/// Whether the path is a file directly in the root directory whose name starts
/// with an uppercase letter. A file inside an uppercase directory doesn't count.
fn is_uppercase_root_file(p: &Path) -> bool {
    let mut components = p.components();
    let (Some(first), None) = (components.next(), components.next()) else {
        return false;
    };
    first
        .as_os_str()
        .to_str()
        .and_then(|x| x.chars().next())
        .is_some_and(|x| x.is_uppercase())
}

pub fn os_path_to_prql_path(path: &Path) -> Result<Vec<String>> {
    // remove file format extension
    let path = path.with_extension("");

    // split by /
    path.components()
        .map(|x| {
            x.as_os_str()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| Error::new_simple(format!("Invalid file path: {path:?}")))
        })
        .try_collect()
}

#[cfg(test)]
mod test {
    use insta::assert_snapshot;

    use super::*;

    fn tree(files: &[(&str, &str)]) -> SourceTree {
        SourceTree::new(
            files
                .iter()
                .map(|(path, content)| (PathBuf::from(path), content.to_string())),
            None,
        )
    }

    fn root_of(tree: &SourceTree) -> Result<String> {
        let files = linearize_tree(tree)?;
        let root = files.last().unwrap();
        assert!(root.module_path.is_empty());
        Ok(root.file_path.to_str().unwrap().to_string())
    }

    #[test]
    fn test_uppercase_root() {
        let tree = tree(&[
            ("Project.prql", "from a"),
            ("other.prql", "from b"),
            ("Sales/orders.prql", "from c"),
        ]);
        assert_snapshot!(root_of(&tree).unwrap(), @"Project.prql");
    }

    #[test]
    fn test_multiple_uppercase_roots() {
        let tree = tree(&[("Project.prql", "from a"), ("Other.prql", "from b")]);
        assert_snapshot!(root_of(&tree).unwrap_err().to_string(), @r#"Error { kind: Error, span: None, reason: Simple("Found more than one root module candidate:\n - Other.prql\n - Project.prql"), hints: ["keep only one file starting with an uppercase letter in the root directory"], code: Some("E0002") }"#);
    }

    #[test]
    fn test_uppercase_directory_is_not_root() {
        let tree = tree(&[("Foo/bar.prql", "from a"), ("b.prql", "from b")]);
        assert_snapshot!(root_of(&tree).unwrap_err().to_string(), @r#"Error { kind: Error, span: None, reason: Simple("Cannot find the root module within the following files:\n - Foo/bar.prql\n - b.prql"), hints: ["add a file that starts with uppercase letter to the root directory"], code: Some("E0002") }"#);
    }
}
