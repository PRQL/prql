use std::fmt::Write;
use std::ops::Range;

use anyhow::{Ok, Result};
use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};

use super::NS_DEFAULT_DB;
use crate::ir::decl::{DeclKind, RootModule, TableDecl, TableExpr};
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
    labeler.fold_table_exprs();

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
    fn fold_table_exprs(&mut self) {
        if let Some(default_db) = self.root_mod.module.names.get(NS_DEFAULT_DB) {
            let default_db = default_db.clone().kind.into_module().unwrap();

            for (_, decl) in default_db.names.into_iter() {
                if let DeclKind::TableDecl(TableDecl {
                    expr: TableExpr::RelationVar(expr),
                    ..
                }) = decl.kind
                {
                    self.fold_expr(*expr).unwrap();
                }
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
                        DeclKind::Column { .. } => Color::Yellow,
                        DeclKind::InstanceOf(_) => Color::Yellow,
                        DeclKind::TableDecl { .. } => Color::Red,
                        DeclKind::Module(_) => Color::Cyan,
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

pub fn debug_call_tree(expr: Expr) -> (Expr, String) {
    let mut collector = CallTreeDebugger {
        indent: 0,
        out: String::new(),
        multiline: true,
    };

    let expr = collector.fold_expr(expr).unwrap();
    (expr, collector.out)
}

/// Traverses AST and collects all node.frame
struct CallTreeDebugger {
    indent: usize,
    multiline: bool,

    out: String,
}

impl CallTreeDebugger {
    fn write<S: ToString>(&mut self, s: S) {
        if self.multiline {
            self.out.write_str(&"  ".repeat(self.indent)).unwrap();
            self.out.write_str(&s.to_string()).unwrap();
        } else {
            self.out.write_str(&s.to_string()).unwrap();
        }
    }

    fn writeln<S: ToString>(&mut self, s: S) {
        if self.multiline {
            self.write(s.to_string() + "\n");
        } else {
            self.write(s);
        }
    }
}

impl PlFold for CallTreeDebugger {
    fn fold_expr_kind(&mut self, expr_kind: ExprKind) -> Result<ExprKind> {
        match expr_kind {
            ExprKind::FuncCall(mut call) => {
                let multiline = self.multiline;
                if !multiline {
                    self.write("(\n");
                    self.indent += 1;
                    self.multiline = true;
                }

                // func name
                self.write("");
                self.multiline = false;
                call.name = Box::new(self.fold_expr(*call.name)?);
                self.multiline = true;
                self.out.write_str(":\n").unwrap();

                // args
                self.indent += 1;
                call.args = self.fold_exprs(call.args)?;
                self.indent -= 1;

                if !multiline {
                    self.indent -= 1;
                    self.write(")");
                }
                self.multiline = multiline;

                Ok(ExprKind::FuncCall(call))
            }
            ExprKind::Ident(ref ident) => {
                self.writeln(ident);
                Ok(expr_kind)
            }
            kind => {
                self.writeln(format!("<{}>", kind.as_ref()));
                Ok(kind)
            }
        }
    }
}
