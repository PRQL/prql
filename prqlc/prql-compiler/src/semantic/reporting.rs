use std::ops::Range;

use anyhow::{Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use crate::ir::decl::{DeclKind, Module, RootModule, TableDecl, TableExpr};
use crate::ir::pl::*;
use crate::Span;

pub fn label_references(root_mod: &RootModule, source_id: String, source: String) -> Vec<u8> {
    let mut report = Report::build(ReportKind::Custom("Info", Color::Blue), &source_id, 0);

    let source = Source::from(source);

    // label all idents and function calls
    let mut labeler = Labeler {
        root_mod,
        source: &source,
        source_id: &source_id,
        report: &mut report,
    };
    labeler.label_module(&labeler.root_mod.module);

    let mut out = Vec::new();
    report
        .finish()
        .write((source_id, source), &mut out)
        .unwrap();
    out
}

/// Traverses AST and add labels for each of the idents and function calls
struct Labeler<'a> {
    root_mod: &'a RootModule,
    source: &'a Source,
    source_id: &'a str,
    report: &'a mut ReportBuilder<'static, (String, Range<usize>)>,
}

impl<'a> Labeler<'a> {
    fn label_module(&mut self, module: &Module) {
        for (_, decl) in module.names.iter() {
            if let DeclKind::TableDecl(TableDecl {
                expr: TableExpr::RelationVar(expr),
                ..
            }) = &decl.kind
            {
                self.fold_expr(*expr.clone()).unwrap();
            }
        }
    }

    fn get_span_lines(&mut self, id: usize) -> Option<String> {
        let decl_span = self.root_mod.span_map.get(&id);
        decl_span.map(|decl_span| {
            let line_span = self.source.get_line_range(&Range::from(*decl_span));
            if line_span.len() <= 1 {
                format!(" at line {}", line_span.start + 1)
            } else {
                format!(" at lines {}-{}", line_span.start + 1, line_span.end)
            }
        })
    }
}

impl<'a> PlFold for Labeler<'a> {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr> {
        if let Some(ident) = node.kind.as_ident() {
            if let Some(span) = node.span {
                let decl = self.root_mod.module.get(ident);

                let ident = format!("[{ident}]");

                let (decl, color) = if let Some(decl) = decl {
                    let color = match &decl.kind {
                        DeclKind::Expr(_) => Color::Blue,
                        DeclKind::Ty(_) => Color::Green,
                        DeclKind::Column { .. } => Color::Yellow,
                        DeclKind::InstanceOf(_) => Color::Yellow,
                        DeclKind::TableDecl { .. } => Color::Red,
                        DeclKind::Module(module) => {
                            self.label_module(module);

                            Color::Cyan
                        }
                        DeclKind::Param(_) => Color::Blue,
                        DeclKind::LayeredModules(_) => Color::Cyan,
                        DeclKind::Infer(_) => Color::White,
                        DeclKind::QueryDef(_) => Color::White,
                    };

                    let location = decl
                        .declared_at
                        .and_then(|id| self.get_span_lines(id))
                        .unwrap_or_default();

                    let decl = match &decl.kind {
                        DeclKind::TableDecl(TableDecl { ty, .. }) => {
                            format!(
                                "table {}",
                                ty.as_ref().and_then(|t| t.name.clone()).unwrap_or_default()
                            )
                        }
                        _ => decl.to_string(),
                    };

                    (format!("{decl}{location}"), color)
                } else if let Some(decl_id) = node.target_id {
                    let lines = self.get_span_lines(decl_id).unwrap_or_default();

                    (format!("variable{lines}"), Color::Yellow)
                } else {
                    ("".to_string(), Color::White)
                };

                self.report.add_label(
                    Label::new((self.source_id.to_string(), Range::from(span)))
                        .with_message(format!("{ident} {decl}"))
                        .with_color(color),
                );
            }
        }
        Ok(Expr {
            kind: self.fold_expr_kind(node.kind)?,
            ..node
        })
    }
}

pub fn collect_frames(expr: Expr) -> Vec<(Span, Lineage)> {
    let mut collector = FrameCollector { frames: vec![] };

    collector.fold_expr(expr).unwrap();

    collector.frames.reverse();
    collector.frames
}

/// Traverses AST and collects all node.frame
struct FrameCollector {
    frames: Vec<(Span, Lineage)>,
}

impl PlFold for FrameCollector {
    fn fold_expr(&mut self, expr: Expr) -> Result<Expr> {
        if matches!(expr.kind, ExprKind::TransformCall(_)) {
            if let Some(span) = expr.span {
                let lineage = expr.lineage.clone();
                if let Some(lineage) = lineage {
                    self.frames.push((span, lineage));
                }
            }
        }

        Ok(Expr {
            kind: self.fold_expr_kind(expr.kind)?,
            ..expr
        })
    }
}
