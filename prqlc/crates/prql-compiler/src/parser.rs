use anyhow::Result;
use std::collections::HashMap;

use crate::utils::IdGenerator;
use crate::{Errors, SourceTree};
use prql_ast::stmt::Stmt;

pub fn parse(file_tree: &SourceTree<String>) -> Result<SourceTree<Vec<Stmt>>> {
    let mut res = SourceTree::default();
    let mut errors = Vec::new();

    let ids: HashMap<_, _> = file_tree.source_ids.iter().map(|(a, b)| (b, a)).collect();
    let mut id_gen = IdGenerator::<usize>::new();

    for (path, source) in &file_tree.sources {
        let id = ids
            .get(path)
            .map(|x| **x)
            .unwrap_or_else(|| id_gen.gen() as u16);

        match prql_parser::parse_source(source, id) {
            Ok(stmts) => {
                res.sources.insert(path.clone(), stmts);
                res.source_ids.insert(id, path.clone());
            }
            Err(errs) => errors.extend(errs),
        }
    }
    if errors.is_empty() {
        Ok(res)
    } else {
        Err(Errors(errors).into())
    }
}
