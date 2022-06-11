use std::ops::Range;

use anyhow::{anyhow, Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use super::{Context, Declaration, Frame};
use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::Span;

pub fn label_references(nodes: &[Node], context: &Context, source_id: String, source: String) {
    let mut report = Report::build(ReportKind::Custom("Info", Color::Blue), &source_id, 0);

    let source = Source::from(source);

    // label all idents and function calls
    let mut labeler = Labeler {
        context,
        source: &source,
        source_id: &source_id,
        report: &mut report,
    };
    // traverse ast
    labeler.fold_nodes(nodes.to_owned()).unwrap();
    // traverse declarations
    // for (d, _) in &context.declarations {
    //     match d {
    //         Declaration::Variable(n) | Declaration::Function(FuncDef { body: n, .. }) => {
    //             labeler.fold_node(*(*n).clone()).unwrap();
    //         }
    //         Declaration::Table(_) => todo!(),
    //     }
    // }

    // label all declarations
    // for (dec, span) in &context.declarations {
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
    context: &'a Context,
    source: &'a Source,
    source_id: &'a str,
    report: &'a mut ReportBuilder<(String, Range<usize>)>,
}

impl<'a> AstFold for Labeler<'a> {
    fn fold_node(&mut self, node: Node) -> Result<Node> {
        if let Some(declared_at) = node.declared_at {
            let (declaration, span) = &self.context.declarations.0[declared_at];
            let message = if let Some(span) = span {
                let span = self.source.get_line_range(&Range::from(*span));
                if span.len() <= 1 {
                    format!("[{declared_at}] {declaration} at line {}", span.start + 1)
                } else {
                    format!(
                        "[{declared_at}] {declaration} at lines {}-{}",
                        span.start + 1,
                        span.end
                    )
                }
            } else {
                declaration.to_string()
            };
            let color = match declaration {
                Declaration::Expression(_) => Color::Blue,
                Declaration::ExternRef { .. } => Color::Cyan,
                Declaration::Table { .. } => Color::Magenta,
                Declaration::Function(_) => Color::Yellow,
            };

            if let Some(span) = node.span {
                self.report.add_label(
                    Label::new((self.source_id.to_string(), Range::from(span)))
                        .with_message(message)
                        .with_color(color),
                );
            }
        }
        Ok(self.fold_item(node.item)?.into())
    }
}

pub fn collect_frames(nodes: Vec<Node>) -> Vec<(Span, Frame)> {
    let mut collector = FrameCollector { frames: vec![] };

    collector.fold_nodes(nodes).unwrap();

    collector.frames
}

/// Traverses AST and collects all node.frame
struct FrameCollector {
    frames: Vec<(Span, Frame)>,
}

impl AstFold for FrameCollector {
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        Ok(table)
    }

    fn fold_pipeline(&mut self, pipeline: Pipeline) -> Result<Pipeline> {
        let mut frame = Frame::default();
        for node in &pipeline.nodes {
            let transform = (node.item)
                .as_transform()
                .ok_or_else(|| anyhow!("plain function in pipeline"))?;

            frame.apply_transform(transform)?;

            self.frames.push((node.span.unwrap(), frame.clone()));
        }

        Ok(pipeline)
    }

    fn fold_func_def(&mut self, function: FuncDef) -> Result<FuncDef> {
        fold_func_def(self, function)
    }
}
