//! Internal machinery for collecting and rendering debug logs.
#![doc(hidden)]

use chrono::prelude::*;
use serde::Serialize;
use std::{sync::RwLock, time::SystemTime};
use strum_macros::AsRefStr;

use prqlc_parser::lexer::lr;
use prqlc_parser::parser::pr;

use crate::ir::{decl, pl, rq};

/// Stores debug info about current compilation.
/// Is reset by [log_start] and [log_finish].
static CURRENT_LOG: RwLock<Option<DebugLog>> = RwLock::new(None);

pub fn log_start() {
    let mut lock = CURRENT_LOG.write().unwrap();
    assert!(lock.is_none());

    let started_at: DateTime<Utc> = SystemTime::now().into();
    let started_at = format!("{}", started_at.format("%+"));

    *lock = Some(DebugLog {
        started_at,
        version: crate::compiler_version().to_string(),
        entries: Vec::new(),

        current_stage: Stage::Parsing(StageParsing::Lexer),
        suppress: false,
    });
}

pub fn log_finish() -> Option<DebugLog> {
    let mut lock = CURRENT_LOG.write().unwrap();
    lock.take()
}

pub fn log_set_suppress(suppress: bool) {
    let mut lock = CURRENT_LOG.write().unwrap();
    if let Some(log) = lock.as_mut() {
        log.suppress = suppress;
    }
}

pub fn log_stage(stage: Stage) {
    let mut lock = CURRENT_LOG.write().unwrap();
    if let Some(log) = lock.as_mut() {
        if log.suppress {
            return;
        }
        log.current_stage = stage;
    }
}

pub fn log_entry(entry: impl FnOnce() -> DebugEntryKind) {
    let mut lock: std::sync::RwLockWriteGuard<'_, Option<DebugLog>> = CURRENT_LOG.write().unwrap();
    if let Some(log) = lock.as_mut() {
        if log.suppress {
            return;
        }

        let entry = DebugEntry {
            stage: log.current_stage,
            kind: entry(),
        };
        log.entries.push(entry);
    }
}

pub fn render_log_to_html<W: std::io::Write>(writer: W, debug_log: &DebugLog) -> core::fmt::Result {
    struct IoWriter<W: std::io::Write> {
        inner: W,
    }

    impl<W: std::io::Write> core::fmt::Write for IoWriter<W> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.inner
                .write(s.as_bytes())
                .map_err(|_| std::fmt::Error)?;
            Ok(())
        }
    }
    let mut io_writer = IoWriter { inner: writer };

    render_html::write_debug_log(&mut io_writer, debug_log)
}

#[derive(Serialize)]
pub struct DebugLog {
    started_at: String,
    version: String,
    entries: Vec<DebugEntry>,

    #[serde(skip)]
    current_stage: Stage,

    #[serde(skip)]
    suppress: bool,
}

#[derive(Serialize)]
struct DebugEntry {
    stage: Stage,
    kind: DebugEntryKind,
}

#[derive(Serialize, AsRefStr)]
pub enum DebugEntryKind {
    ReprPrql(crate::SourceTree),
    ReprLr(lr::Tokens),
    ReprPr(pr::ModuleDef),
    ReprPl(pl::ModuleDef),
    ReprDecl(decl::RootModule),
    ReprRq(rq::RelationalQuery),
    ReprSql(String),
    // TODO: maybe route all log::debug and friends here?
    // Message(String),
}

#[derive(Clone, Copy, Serialize, AsRefStr)]
pub enum Stage {
    Initial,
    Parsing(StageParsing),
    Semantic(StageSemantic),
    Sql,
}

impl Stage {
    fn sub_stage(&self) -> Option<&'_ str> {
        match self {
            Stage::Initial => None,
            Stage::Parsing(s) => Some(s.as_ref()),
            Stage::Semantic(s) => Some(s.as_ref()),
            Stage::Sql => None,
        }
    }
}

#[derive(Clone, Copy, Serialize, AsRefStr)]
pub enum StageParsing {
    Lexer,
    Parser,
}

#[derive(Clone, Copy, Serialize, AsRefStr)]
pub enum StageSemantic {
    AstExpand,
    Resolver,
    Lowering,
}

mod render_html {
    use crate::SourceTree;

    use super::*;

    use std::{
        collections::HashMap,
        fmt::{Debug, Result, Write},
    };

    pub fn write_debug_log<W: Write>(w: &mut W, debug_log: &DebugLog) -> Result {
        writeln!(w, "<!DOCTYPE html>")?;

        writeln!(w, "<head>")?;
        writeln!(w, "<title>prqlc debug log {}</title>", debug_log.started_at)?;
        writeln!(w, "<style>{}</style>", CSS_STYLES)?;
        writeln!(w, "</head>")?;
        writeln!(w, "<body>")?;

        // header
        writeln!(w, "<header>")?;
        writeln!(w, "<h1>prqlc debug log</h1>")?;

        write_key_values(
            w,
            &[
                ("started_at", &debug_log.started_at),
                ("version", &debug_log.version),
            ],
        )?;

        writeln!(w, "</header>")?;

        // entires
        writeln!(w, "<div class=\"entries\">")?;
        for (index, entry) in debug_log.entries.iter().enumerate() {
            let entry_id = format!("entry-{index}");

            let stage = entry.stage.as_ref().to_lowercase();
            let substage = entry
                .stage
                .sub_stage()
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            writeln!(w, "<div class=\"entry {stage} {substage}\">",)?;

            writeln!(
                w,
                "<input id=\"{entry_id}\" class=\"toggle\" type=\"checkbox\" checked>"
            )?;
            writeln!(
                w,
                "<label for=\"{entry_id}\" class=\"toggle-label\">{}</label>",
                entry.kind.as_ref()
            )?;
            writeln!(w, "<div class=\"collapsible entry-content\">")?;
            match &entry.kind {
                DebugEntryKind::ReprPrql(a) => write_repr_prql(w, a)?,
                DebugEntryKind::ReprLr(a) => write_repr_lr(w, a)?,
                DebugEntryKind::ReprPr(a) => write_repr_pr(w, a)?,
                DebugEntryKind::ReprPl(a) => write_repr_pl(w, a)?,
                DebugEntryKind::ReprDecl(a) => write_repr_decl(w, a)?,
                DebugEntryKind::ReprRq(a) => write_repr_rq(w, a)?,
                DebugEntryKind::ReprSql(a) => write_repr_sql(w, a)?,
            }
            writeln!(w, "</div>")?; // collapsible
            writeln!(w, "</div>")?; // entry
        }
        writeln!(w, "</div>")?; // entries

        writeln!(w, "</body>")?;

        Ok(())
    }

    pub fn write_repr_prql<W: Write>(w: &mut W, source_tree: &SourceTree) -> Result {
        writeln!(w, "<div class=\"prql repr\">")?;

        write_key_values(w, &[("root", &source_tree.root)])?;

        let reverse_ids: HashMap<_, _> = source_tree
            .source_ids
            .iter()
            .map(|(id, path)| (path, id))
            .collect();

        for (path, source) in &source_tree.sources {
            writeln!(w, "<div class=\"source indent\">")?;

            write_key_values(w, &[("path", &path), ("source_id", &reverse_ids.get(path))])?;

            writeln!(w, "<code>{source}</code>")?;
            writeln!(w, "</div>")?; // source
        }

        writeln!(w, "</div>")
    }

    pub fn write_repr_lr<W: Write>(w: &mut W, tokens: &lr::Tokens) -> Result {
        writeln!(w, "<div class=\"lr repr\">")?;

        for token in &tokens.0 {
            writeln!(
                w,
                "<token span=\"{}:{}\">",
                token.span.start, token.span.end
            )?;
            writeln!(w, "{:?}", token.kind)?;
            writeln!(w, "</token>")?;
        }

        writeln!(w, "</div>")
    }

    pub fn write_repr_pr<W: Write>(w: &mut W, root_mod: &pr::ModuleDef) -> Result {
        writeln!(w, "<div class=\"pr repr\">")?;
        writeln!(w, "<code>{:#?}</code>", root_mod)?;
        writeln!(w, "</div>")
    }

    pub fn write_repr_pl<W: Write>(w: &mut W, root_mod: &pl::ModuleDef) -> Result {
        writeln!(w, "<div class=\"pl repr\">")?;
        writeln!(w, "<code>{:#?}</code>", root_mod)?;
        writeln!(w, "</div>")
    }

    pub fn write_repr_decl<W: Write>(w: &mut W, root_mod: &decl::RootModule) -> Result {
        writeln!(w, "<div class=\"decl repr\">")?;
        writeln!(w, "<code>{:#?}</code>", root_mod)?;
        writeln!(w, "</div>")
    }

    pub fn write_repr_rq<W: Write>(w: &mut W, query: &rq::RelationalQuery) -> Result {
        writeln!(w, "<div class=\"decl repr\">")?;
        writeln!(w, "<code>{:#?}</code>", query)?;
        writeln!(w, "</div>")
    }

    pub fn write_repr_sql<W: Write>(w: &mut W, query: &str) -> Result {
        writeln!(w, "<div class=\"sql repr\">")?;
        writeln!(w, "<code>{}</code>", query)?;
        writeln!(w, "</div>")
    }

    pub fn write_key_values<W: Write>(w: &mut W, pairs: &[(&'static str, &dyn Debug)]) -> Result {
        writeln!(w, "<div class=\"key_values\">")?;
        for (k, v) in pairs {
            writeln!(w, "<div><b>{k}</b>: {v:?}</div>")?;
        }
        writeln!(w, "</div>")
    }

    const CSS_STYLES: &str = r#"
    body {
        font-size: 12px;
        font-family: monospace;
        color: #f4f2ee;
        background-color: #191b1c;
    }
    
    .key_values {
        display: flex;
        gap: 1em;
    }
    
    .entries {
        direction: flex;
        flex-direction: column;
    }
    
    .entry {
        direction: block;
        padding: 0.5em 0 0.5em 0;
    }
    
    .entry>label {
        margin: 0;
        font-size: 18px;
        padding: 1em 0 0 0;
    }
    
    .entry>.toggle {
        display: none;
    }
    .entry>.toggle-label {
        display: block;
        cursor: pointer;
        text-decoration: underline;
    }
    .entry>.toggle-label:hover {
        color: purple;
    }
    .entry>.toggle:checked + .toggle-label + .collapsible {
      display: none;
    }

    .entry-content {
        display: flex;
        flex-direction: column;
    }

    code {
        white-space: preserve;
        word-break: keep-all;
    }
    .indent {
      margin-top: 0.5em;
      padding-left: 1em;
      border-left: 1px solid gray;
    }      
    "#;
}
