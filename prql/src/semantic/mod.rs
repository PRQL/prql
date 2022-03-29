pub mod analyzer;
pub mod reporting;

use anyhow::Result;

use self::analyzer::{Declaration, SemanticAnalyzer, VarDec};
use super::ast::*;

pub fn analyze(ast: Node) -> Result<SemanticAnalyzer> {
    let mut analyzer = SemanticAnalyzer::new();

    analyzer.append(ast.item.into_query().unwrap())?;

    Ok(analyzer)
}
