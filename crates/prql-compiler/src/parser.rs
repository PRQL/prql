use std::collections::HashMap;

use anyhow::Result;
use prql_ast::stmt::Stmt;

use crate::utils::IdGenerator;
use crate::SourceTree;
use crate::{Error, Errors, Reason, WithErrorInfo};

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
    let stmts = prql_parser::parse_source(source, source_id).map_err(|errors| {
        Errors(
            errors
                .into_iter()
                .map(|err| {
                    // TODO: we actually want to to avoid this stringification
                    // here but it's currently just there because Reason
                    // currently must implement Clone but we don't necessarily want prql_parser::Error to implement Clone.
                    Error::new(Reason::Simple(err.kind.to_string())).with_span(Some(err.span))
                })
                .collect(),
        )
    })?;

    Ok(stmts)
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use prql_ast::stmt::Stmt;

    /// Helper that does not track source_ids
    #[cfg(test)]
    pub fn parse_single(source: &str) -> anyhow::Result<Vec<Stmt>> {
        super::parse_source(source, 0)
    }

    #[test]
    fn test_error_unicode_string() {
        // Test various unicode strings successfully parse errors. We were
        // getting loops in the lexer before.
        parse_single("s’ ").unwrap_err();
        parse_single("s’").unwrap_err();
        parse_single(" s’").unwrap_err();
        parse_single(" ’ s").unwrap_err();
        parse_single("’s").unwrap_err();
        parse_single("👍 s’").unwrap_err();

        let source = "Mississippi has four S’s and four I’s.";
        assert_debug_snapshot!(parse_single(source).unwrap_err(), @r###"
        Errors(
            [
                Error {
                    kind: Error,
                    span: Some(
                        0:22-23,
                    ),
                    reason: Simple(
                        "unexpected ’",
                    ),
                    hints: [],
                    code: None,
                },
                Error {
                    kind: Error,
                    span: Some(
                        0:35-36,
                    ),
                    reason: Simple(
                        "unexpected ’",
                    ),
                    hints: [],
                    code: None,
                },
                Error {
                    kind: Error,
                    span: Some(
                        0:37-38,
                    ),
                    reason: Simple(
                        "Expected * or an identifier, but didn't find anything before the end.",
                    ),
                    hints: [],
                    code: None,
                },
            ],
        )
        "###);
    }

    #[test]
    fn test_error_unexpected() {
        assert_debug_snapshot!(parse_single("Answer: T-H-A-T!").unwrap_err(), @r###"
        Errors(
            [
                Error {
                    kind: Error,
                    span: Some(
                        0:6-7,
                    ),
                    reason: Simple(
                        "unexpected : while parsing source file",
                    ),
                    hints: [],
                    code: None,
                },
            ],
        )
        "###);
    }
}
