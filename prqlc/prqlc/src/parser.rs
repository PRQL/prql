use std::path::PathBuf;
use std::{collections::HashMap, path::Path};

use itertools::Itertools;

use crate::debug;
use crate::pr;
use crate::{Error, Errors, Result, SourceTree, WithErrorInfo};

pub fn parse(file_tree: &SourceTree) -> Result<pr::ModuleDef, Errors> {
    // register a new stage of the compiler
    // (here should register lexer stage first, but that all happens in a single call to prqlc_parser)
    debug::log_entry(|| debug::DebugEntryKind::ReprPrql(file_tree.clone()));
    debug::log_stage(debug::Stage::Parsing(debug::StageParsing::Parser));

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

/// Build PRQL AST from a PRQL query string.
pub(crate) fn parse_source(source: &str, source_id: u16) -> Result<Vec<pr::Stmt>, Vec<Error>> {
    let (tokens, mut errors) = prqlc_parser::lexer::lex_source_recovery(source, source_id);

    let ast = if let Some(tokens) = tokens {
        let (ast, parse_errors) = prqlc_parser::parser::parse_lr_to_pr(source, source_id, tokens);
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

fn linearize_tree(tree: &SourceTree) -> Result<Vec<SourceFile>> {
    // find root
    let root_path;

    if tree.sources.len() == 1 {
        // if there is only one file, use that as the root
        root_path = tree.sources.keys().next().unwrap();
    } else if let Some(root) = tree.sources.get_key_value(&PathBuf::from("")) {
        // if there is an empty path, that's the root
        root_path = root.0;
    } else if let Some(root) = tree.sources.keys().find(path_starts_with_uppercase) {
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
    stmt.kind.as_module_def().map_or(false, |x| x.name == name)
}

fn path_starts_with_uppercase(p: &&PathBuf) -> bool {
    p.components()
        .next()
        .and_then(|x| x.as_os_str().to_str())
        .and_then(|x| x.chars().next())
        .map_or(false, |x| x.is_uppercase())
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
