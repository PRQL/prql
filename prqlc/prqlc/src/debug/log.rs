//! Internal machinery for collecting and rendering debug logs.
#![doc(hidden)]

use chrono::prelude::*;
use serde::Serialize;
use std::{sync::RwLock, time::SystemTime};
use strum_macros::AsRefStr;

use crate::ir::{decl, pl, rq};
use prqlc_parser::lexer::lr;
use prqlc_parser::parser::pr;

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

#[derive(Serialize)]
pub struct DebugLog {
    pub(super) started_at: String,
    pub(super) version: String,
    pub(super) entries: Vec<DebugEntry>,

    #[serde(skip)]
    current_stage: Stage,

    #[serde(skip)]
    suppress: bool,
}

#[derive(Serialize)]
pub(super) struct DebugEntry {
    pub(crate) stage: Stage,
    pub(crate) kind: DebugEntryKind,
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
    pub(super) fn sub_stage(&self) -> Option<&'_ str> {
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
