use std::collections::HashMap;
use std::ops::Range;

use ariadne::{Color, Label, Report, ReportBuilder, ReportKind, Source};
use schemars::JsonSchema;
use serde::Serialize;

use crate::ir::decl::{DeclKind, Module, RootModule, TableDecl, TableExpr};
use crate::ir::pl;
use crate::ir::pl::PlFold;
use crate::pr;
use crate::{Result, Span};

pub fn label_references(root_mod: &RootModule, source_id: String, source: String) -> Vec<u8> {
    let report_span = (source_id.clone(), 0..source.len());

    let mut report = Report::build(ReportKind::Custom("Info", Color::Blue), report_span);

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

impl Labeler<'_> {
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

impl pl::PlFold for Labeler<'_> {
    fn fold_expr(&mut self, node: pl::Expr) -> Result<pl::Expr> {
        if let Some(ident) = node.kind.as_ident() {
            if let Some(span) = node.span {
                let decl = self.root_mod.module.get(ident);

                let ident = format!("[{ident}]");

                let (decl, color) = if let Some(decl) = decl {
                    let color = match &decl.kind {
                        DeclKind::Expr(_) => Color::Blue,
                        DeclKind::Ty(_) => Color::Green,
                        DeclKind::Column { .. } => Color::Yellow,
                        DeclKind::InstanceOf(_, _) => Color::Yellow,
                        DeclKind::TableDecl { .. } => Color::Red,
                        DeclKind::Module(module) => {
                            self.label_module(module);

                            Color::Cyan
                        }
                        DeclKind::LayeredModules(_) => Color::Cyan,
                        DeclKind::Infer(_) => Color::White,
                        DeclKind::QueryDef(_) => Color::White,
                        DeclKind::Import(_) => Color::White,
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

                let label_span = (self.source_id.to_string(), span.start..span.end);

                self.report.add_label(
                    Label::new(label_span)
                        .with_message(format!("{ident} {decl}"))
                        .with_color(color),
                );
            }
        }
        Ok(pl::Expr {
            kind: self.fold_expr_kind(node.kind)?,
            ..node
        })
    }
}

/// Traverses AST and collects all node.frame
pub fn collect_frames(expr: pl::Expr) -> FrameCollector {
    let mut collector = FrameCollector {
        frames: vec![],
        nodes: vec![],
        ast: None,
    };

    collector.fold_expr(expr).unwrap();

    collector.frames.reverse();

    let mut parent_updates = Vec::new();
    let mut node_pos = HashMap::new();
    for (i, node) in collector.nodes.iter().enumerate() {
        node_pos.insert(node.id, i);
        for &child in &node.children {
            parent_updates.push((child, node.id));
        }
    }

    for (child, parent) in parent_updates {
        if let Some(child_pos) = node_pos.get(&child) {
            if let Some(child_node) = collector.nodes.get_mut(*child_pos) {
                child_node.parent = Some(parent);
            }
        }
    }

    collector
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ExprGraphNode {
    /// Node unique ID
    pub id: usize,

    /// Descriptive text about the node
    pub kind: String,

    /// Position of this expr in the original source query
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,

    /// When this node is part of a Tuple, this holds the alias name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,

    /// When kind is Ident, this holds the referenced name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ident: Option<pl::ExprKind>,

    /// Upstream sources of data for this expr as node IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<usize>,

    /// If this expr holds other exprs, these are their node IDs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<usize>,

    /// If this expr is inside of another expr, this is its parent node ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
pub struct FrameCollector {
    /// Each transformation step in the main pipeline corresponds to a single
    /// frame. This holds the output columns at each frame, as well as the span
    /// position of the frame.
    pub frames: Vec<(Option<Span>, pl::Lineage)>,

    /// A mapping of expression graph node IDs to their node definitions.
    pub nodes: Vec<ExprGraphNode>,

    /// The parsed AST from the provided query.
    pub ast: Option<pr::ModuleDef>,
}

impl PlFold for FrameCollector {
    fn fold_expr(&mut self, expr: pl::Expr) -> Result<pl::Expr> {
        if let Some(id) = expr.id {
            let targets = match &expr.kind {
                pl::ExprKind::Ident(_) => {
                    if let Some(target_id) = expr.target_id {
                        vec![target_id]
                    } else {
                        vec![]
                    }
                }
                pl::ExprKind::RqOperator { args, .. } => args.iter().filter_map(|e| e.id).collect(),
                pl::ExprKind::Case(switch) => switch
                    .iter()
                    .flat_map(|c| vec![c.condition.id.unwrap(), c.value.id.unwrap()])
                    .collect(),
                pl::ExprKind::SString(iv) | pl::ExprKind::FString(iv) => iv
                    .iter()
                    .filter_map(|i| match i {
                        pl::InterpolateItem::Expr { expr: e, .. } => e.id,
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            };

            let ident = if matches!(&expr.kind, pl::ExprKind::Ident(_)) {
                Some(expr.kind.clone())
            } else {
                None
            };

            let children = match &expr.kind {
                pl::ExprKind::Tuple(args) | pl::ExprKind::Array(args) => {
                    args.iter().filter_map(|e| e.id).collect()
                }
                pl::ExprKind::TransformCall(tc) => {
                    let mut tcc = vec![tc.input.id.unwrap()];

                    match *tc.kind {
                        pl::TransformKind::Derive { assigns: ref e }
                        | pl::TransformKind::Select { assigns: ref e }
                        | pl::TransformKind::Filter { filter: ref e }
                        | pl::TransformKind::Append(ref e)
                        | pl::TransformKind::Loop(ref e)
                        | pl::TransformKind::Group {
                            pipeline: ref e, ..
                        }
                        | pl::TransformKind::Window {
                            pipeline: ref e, ..
                        } => {
                            tcc.push(e.id.unwrap());
                        }
                        pl::TransformKind::Aggregate { assigns: ref e } => {
                            tcc.push(e.id.unwrap());
                            if let Some(p) = &tc.partition {
                                tcc.push(p.id.unwrap())
                            }
                        }
                        pl::TransformKind::Join {
                            ref with,
                            ref filter,
                            ..
                        } => {
                            tcc.push(with.id.unwrap());
                            tcc.push(filter.id.unwrap());
                        }
                        pl::TransformKind::Take { ref range } => {
                            if let Some(e) = &range.start {
                                tcc.push(e.id.unwrap());
                            }
                            if let Some(e) = &range.end {
                                tcc.push(e.id.unwrap());
                            }
                        }
                        pl::TransformKind::Sort { ref by } => {
                            for c in by {
                                tcc.push(c.column.id.unwrap());
                            }
                        }
                    };

                    tcc
                }
                _ => vec![],
            };

            let kind = match &expr.kind {
                pl::ExprKind::TransformCall(tc) => {
                    let tc_kind = tc.kind.as_ref().as_ref().to_string();

                    format!("TransformCall: {tc_kind}")
                }
                _ => expr.kind.as_ref().to_string(),
            };

            self.nodes.push(ExprGraphNode {
                id,
                kind,
                span: expr.span,
                alias: expr.alias.clone(),
                ident,
                targets,
                children,
                parent: None,
            });
        }

        self.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        self.nodes.dedup();

        if matches!(expr.kind, pl::ExprKind::TransformCall(_)) {
            let lineage = expr.lineage.clone();
            if let Some(lineage) = lineage {
                self.frames.push((expr.span, lineage));
            }
        }

        Ok(pl::Expr {
            kind: self.fold_expr_kind(expr.kind)?,
            ..expr
        })
    }
}
