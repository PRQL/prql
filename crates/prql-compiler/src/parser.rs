use std::collections::HashMap;

use anyhow::Result;
use prql_ast::stmt::Stmt;

use crate::utils::IdGenerator;
use crate::Errors;
use crate::SourceTree;

pub fn parse(file_tree: &SourceTree<String>) -> Result<SourceTree<Vec<Stmt>>> {
    let mut res = SourceTree::default();

    let ids: HashMap<_, _> = file_tree.source_ids.iter().map(|(a, b)| (b, a)).collect();
    let mut id_gen = IdGenerator::<usize>::new();

    for (path, source) in &file_tree.sources {
        let id = ids
            .get(path)
            .map(|x| **x)
            .unwrap_or_else(|| id_gen.gen() as u16);
        let stmts = parse_source(source, id)?;

        res.sources.insert(path.clone(), stmts);
        res.source_ids.insert(id, path.clone());
    }
    Ok(res)
}

fn parse_source(source: &str, source_id: u16) -> Result<Vec<prql_ast::stmt::Stmt>> {
    let stmts = prql_parser::parse_source(source, source_id).map_err(Errors)?;

    Ok(stmts)
}
