//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::{HashMap, HashSet};
use std::iter::zip;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::ast::pl::TableExternRef;
use crate::ast::rq::{
    fold_table, CId, Compute, Query, RelationColumn, RelationKind, RqFold, TId, TableDecl,
    TableRef, Transform,
};
use crate::utils::{IdGenerator, NameGenerator};

use super::preprocess::SqlTransform;

#[derive(Default)]
pub struct AnchorContext {
    pub(super) column_decls: HashMap<CId, ColumnDecl>,
    pub(super) column_names: HashMap<CId, String>,

    pub(super) table_decls: HashMap<TId, TableDecl>,

    pub(super) table_instances: HashMap<TIId, TableRef>,

    pub(super) col_name: NameGenerator,
    pub(super) table_name: NameGenerator,

    pub(super) cid: IdGenerator<CId>,
    pub(super) tid: IdGenerator<TId>,
    pub(super) tiid: IdGenerator<TIId>,
}
/// Table instance id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TIId(usize);

impl From<usize> for TIId {
    fn from(id: usize) -> Self {
        TIId(id)
    }
}

/// Column declaration.
#[derive(Debug, PartialEq, Clone, strum::AsRefStr, EnumAsInner)]
pub enum ColumnDecl {
    RelationColumn(TIId, CId, RelationColumn),
    Compute(Box<Compute>),
}

impl AnchorContext {
    pub fn of(query: Query) -> (Self, Query) {
        let (cid, tid, query) = IdGenerator::load(query);

        let context = AnchorContext {
            cid,
            tid,
            tiid: IdGenerator::new(),
            col_name: NameGenerator::new("_expr_"),
            table_name: NameGenerator::new("table_"),
            ..Default::default()
        };
        QueryLoader::load(context, query)
    }

    pub fn register_wildcard(&mut self, tiid: TIId) -> CId {
        let id = self.cid.gen();
        let kind = ColumnDecl::RelationColumn(tiid, id, RelationColumn::Wildcard);
        self.column_decls.insert(id, kind);
        id
    }

    pub fn register_compute(&mut self, compute: Compute) {
        let id = compute.id;
        let decl = ColumnDecl::Compute(Box::new(compute));
        self.column_decls.insert(id, decl);
    }

    pub fn create_table_instance(&mut self, mut table_ref: TableRef) {
        let tiid = self.tiid.gen();

        for (col, cid) in &table_ref.columns {
            let def = ColumnDecl::RelationColumn(tiid, *cid, col.clone());
            self.column_decls.insert(*cid, def);
        }

        if table_ref.name.is_none() {
            table_ref.name = Some(self.table_name.gen())
        }

        self.table_instances.insert(tiid, table_ref);
    }

    pub(crate) fn ensure_column_name(&mut self, cid: CId) -> Option<&String> {
        // don't name wildcards & named RelationColumns
        let decl = &self.column_decls[&cid];
        if let ColumnDecl::RelationColumn(_, _, col) = decl {
            match col {
                RelationColumn::Single(Some(name)) => {
                    let entry = self.column_names.entry(cid);
                    return Some(entry.or_insert_with(|| name.clone()));
                }
                RelationColumn::Wildcard => return None,
                _ => {}
            }
        }

        let entry = self.column_names.entry(cid);
        Some(entry.or_insert_with(|| self.col_name.gen()))
    }

    pub(super) fn load_names(
        &mut self,
        pipeline: &[SqlTransform],
        output_cols: Vec<RelationColumn>,
    ) {
        let output_cids = Self::determine_select_columns(pipeline);

        assert_eq!(output_cids.len(), output_cols.len());

        for (cid, col) in zip(output_cids.iter(), output_cols) {
            if let RelationColumn::Single(Some(name)) = col {
                self.column_names.insert(*cid, name);
            }
        }
    }

    pub(super) fn determine_select_columns(pipeline: &[SqlTransform]) -> Vec<CId> {
        use SqlTransform::*;
        use Transform::*;

        if let Some((last, remaining)) = pipeline.split_last() {
            match last {
                Super(From(table)) => table.columns.iter().map(|(_, cid)| *cid).collect(),
                Super(Join { with: table, .. }) => [
                    Self::determine_select_columns(remaining),
                    table.columns.iter().map(|(_, cid)| *cid).collect_vec(),
                ]
                .concat(),
                Super(Select(cols)) => cols.clone(),
                Super(Aggregate { partition, compute }) => {
                    [partition.clone(), compute.clone()].concat()
                }
                _ => Self::determine_select_columns(remaining),
            }
        } else {
            Vec::new()
        }
    }

    /// Returns a set of all columns of all tables in a pipeline
    pub(super) fn collect_pipeline_inputs(
        &self,
        pipeline: &[SqlTransform],
    ) -> (Vec<TIId>, HashSet<CId>) {
        let mut tables = Vec::new();
        let mut columns = HashSet::new();
        for t in pipeline {
            if let SqlTransform::Super(
                Transform::From(table) | Transform::Join { with: table, .. },
            ) = t
            {
                // a hack to get TIId of a TableRef
                // (ideally, TIId would be saved in TableRef)
                if let Some((_, cid)) = table.columns.first() {
                    tables.push(*self.column_decls[cid].as_relation_column().unwrap().0);
                } else {
                    panic!("table without columns?")
                }

                columns.extend(table.columns.iter().map(|(_, cid)| cid));
            }
        }
        (tables, columns)
    }

    pub(super) fn contains_wildcard(&self, cids: &[CId]) -> bool {
        for cid in cids {
            let decl = &self.column_decls[cid];
            if let ColumnDecl::RelationColumn(_, _, RelationColumn::Wildcard) = decl {
                return true;
            }
        }
        false
    }
}

/// Loads info about [Query] into [AnchorContext]
struct QueryLoader {
    context: AnchorContext,
}

impl QueryLoader {
    fn load(context: AnchorContext, query: Query) -> (AnchorContext, Query) {
        let mut loader = QueryLoader { context };
        let query = loader.fold_query(query).unwrap();
        (loader.context, query)
    }
}

impl RqFold for QueryLoader {
    fn fold_table(&mut self, table: TableDecl) -> Result<TableDecl> {
        let mut decl = fold_table(self, table)?;

        if let RelationKind::ExternRef(TableExternRef::LocalTable(table)) = &decl.relation.kind {
            decl.name = Some(table.clone());
        }

        if decl.name.is_none() && decl.relation.kind.as_extern_ref().is_none() {
            decl.name = Some(self.context.table_name.gen());
        }

        self.context.table_decls.insert(decl.id, decl.clone());
        Ok(decl)
    }

    fn fold_compute(&mut self, compute: Compute) -> Result<Compute> {
        self.context.register_compute(compute.clone());
        Ok(compute)
    }

    fn fold_table_ref(&mut self, mut table_ref: TableRef) -> Result<TableRef> {
        let tiid = self.context.tiid.gen();

        if table_ref.name.is_none() {
            table_ref.name = Some(self.context.table_name.gen());
        }

        // store
        self.context.table_instances.insert(tiid, table_ref.clone());

        for (col, cid) in &table_ref.columns {
            self.context
                .column_decls
                .insert(*cid, ColumnDecl::RelationColumn(tiid, *cid, col.clone()));
        }

        Ok(table_ref)
    }
}
