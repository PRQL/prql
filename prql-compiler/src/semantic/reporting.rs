use std::ops::Range;

use anyhow::{Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use super::{Context, Declaration, Frame};
use crate::ast::ast_fold::*;
use crate::ast::*;
use crate::error::Span;

pub fn label_references(
    stmts: Vec<Stmt>,
    context: &Context,
    source_id: String,
    source: String,
) -> (Vec<u8>, Vec<Stmt>) {
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
    let stmts = labeler.fold_stmts(stmts).unwrap();

    let mut out = Vec::new();
    report
        .finish()
        .write((source_id, source), &mut out)
        .unwrap();
    (out, stmts)
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

pub fn collect_frames(stmts: Vec<Stmt>) -> Vec<(Span, Frame)> {
    let mut collector = FrameCollector { frames: vec![] };

    collector.fold_stmts(stmts).unwrap();

    collector.frames
}

/// Traverses AST and collects all node.frame
struct FrameCollector {
    frames: Vec<(Span, Frame)>,
}

impl AstFold for FrameCollector {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        if let ExprKind::TransformCall(tc) = &expr.kind {
            let span = match tc.kind.as_ref() {
                TransformKind::From(expr) => expr.span.unwrap(),
                TransformKind::Derive { tbl, .. } |
                TransformKind::Select { tbl, .. } |
                TransformKind::Filter { tbl, .. } |
                TransformKind::Aggregate { tbl, .. } |
                TransformKind::Sort { tbl, .. } |
                TransformKind::Take { tbl, .. } |
                TransformKind::Join { tbl, .. } |
                TransformKind::Group { tbl, .. } |
                TransformKind::Window { tbl, .. } => tbl.span.unwrap()
            };

            let frame = expr.ty.clone().and_then(|t| t.into_table().ok());
            if let Some(frame) = frame {
                self.frames.push((span, frame));
            }
        }

        let mut expr = expr;
        expr.kind = self.fold_expr_kind(expr.kind)?;
        Ok(expr)
    }
}
