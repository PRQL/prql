use std::ops::Range;

use anyhow::{Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use super::{Declaration, SemanticAnalyzer, VarDec};
use crate::ast::FuncDef;
use crate::internals::{AstFold, Node};

pub fn print(analyzer: &SemanticAnalyzer, source_id: String, source: String) {
    let mut report = Report::build(ReportKind::Custom("Info", Color::Blue), &source_id, 0);

    let source = Source::from(source);

    // label all idents and function calls
    let mut labeler = Labeler {
        analyzer,
        source: &source,
        source_id: &source_id,
        report: &mut report,
    };
    // traverse ast
    labeler.fold_node(analyzer.get_ast().clone()).unwrap();
    // traverse declarations
    for (d, _) in &analyzer.declarations {
        match d {
            Declaration::Variable(VarDec { declaration: n, .. })
            | Declaration::Function(FuncDef { body: n, .. }) => {
                labeler.fold_node(*(*n).clone()).unwrap();
            }
            Declaration::Table(_) => todo!(),
        }
    }

    // label all declarations
    // for (dec, span) in &analyzer.declarations {
    //     if let Some(span) = span {
    //         report.add_label(
    //             Label::new((source_id.clone(), Range::from(*span)))
    //                 .with_message(dec.to_string()),
    //         );
    //     }
    // }

    report.finish().print((source_id, source)).unwrap();
}

/// Traverses AST and add labels for each of the idents and function calls
struct Labeler<'a> {
    analyzer: &'a SemanticAnalyzer,
    source: &'a Source,
    source_id: &'a str,
    report: &'a mut ReportBuilder<(String, Range<usize>)>,
}

impl<'a> AstFold for Labeler<'a> {
    fn fold_node(&mut self, node: Node) -> Result<Node> {
        if let Some(declared_at) = node.declared_at {
            let (declaration, span) = &self.analyzer.declarations[declared_at];
            let message = if let Some(span) = span {
                let span = self.source.get_line_range(&Range::from(*span));
                if span.len() <= 1 {
                    format!("{declaration} at line {}", span.start + 1)
                } else {
                    format!("{declaration} at lines {}-{}", span.start + 1, span.end)
                }
            } else {
                declaration.to_string()
            };
            let color = match declaration {
                Declaration::Variable(_) => Color::Blue,
                Declaration::Table(_) => Color::Magenta,
                Declaration::Function(_) => Color::Yellow,
            };

            self.report.add_label(
                Label::new((self.source_id.to_string(), Range::from(node.span)))
                    .with_message(message)
                    .with_color(color),
            )
        }
        Ok(self.fold_item(node.item)?.into())
    }
}
