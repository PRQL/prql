use std::ops::Range;

use anyhow::{Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use super::{Context, Declaration, Frame};
use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::Span;
use crate::ir::{IrFold, Query, Transform};

pub fn label_references(query: Query, context: &Context, source_id: String, source: String) {
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
    labeler.fold_transforms(query.main_pipeline).unwrap();
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
    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if let Some(declared_at) = node.declared_at {
            let (declaration, span) = &self.context.declarations.decls[declared_at];
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
        Ok(self.fold_expr_kind(node.kind)?.into())
    }
}

impl<'a> IrFold for Labeler<'a> {}

pub fn collect_frames(query: Query) -> Vec<(Span, Frame)> {
    let mut collector = FrameCollector { frames: vec![] };

    collector.fold_query(query).unwrap();

    collector.frames
}

/// Traverses AST and collects all node.frame
struct FrameCollector {
    frames: Vec<(Span, Frame)>,
}

impl AstFold for FrameCollector {}

impl IrFold for FrameCollector {
    fn fold_transform(&mut self, transform: Transform) -> Result<Transform> {
        // TODO: fix this
        // let span = transform.span.expect("transform without a span?");
        // self.frames.push((span, transform.ty.clone()));
        Ok(transform)
    }
}
