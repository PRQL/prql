//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::{HashMap, HashSet};
use std::iter::zip;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;

use crate::ast::rq::{
    fold_table, CId, Compute, Query, Relation, RelationColumn, RelationKind, RqFold, TId,
    TableDecl, TableRef, Transform,
};
use crate::utils::{IdGenerator, NameGenerator};

use super::preprocess::{SqlRelation, SqlTransform};

/// The AnchorContext struct stores information about tables and columns, and
/// is used to generate new IDs and names.
#[derive(Default, Debug)]
pub struct AnchorContext {
    pub(super) column_decls: HashMap<CId, ColumnDecl>,
    pub(super) column_names: HashMap<CId, String>,

    pub(super) table_decls: HashMap<TId, SqlTableDecl>,

    pub(super) table_instances: HashMap<TIId, TableRef>,

    pub(super) col_name: NameGenerator,
    pub(super) table_name: NameGenerator,

    pub(super) cid: IdGenerator<CId>,
    pub(super) tid: IdGenerator<TId>,
    pub(super) tiid: IdGenerator<TIId>,
}

/// The [SqlTableDecl] struct contains information about a table declaration,
/// including its ID, name, and relation (if it has been defined).
#[derive(Debug, Clone)]
pub(super) struct SqlTableDecl {
    #[allow(dead_code)]
    pub id: TId,

    pub name: Option<String>,

    /// Relation that still needs to be defined (usually as CTE) so it can be referenced by name.
    /// None means that it has already been defined, or was not needed to be defined in the
    /// first place.
    pub relation: Option<SqlRelation>,
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
    /// Returns a new AnchorContext object based on a Query object. This method
    /// generates new IDs and names for tables and columns as needed.
    pub fn of(query: Query) -> (Self, Relation) {
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

    /// Generates a new ID and name for a wildcard column and registers it in the
    /// AnchorContext's column_decls HashMap.
    pub fn register_wildcard(&mut self, tiid: TIId) -> CId {
        let id = self.cid.gen();
        let kind = ColumnDecl::RelationColumn(tiid, id, RelationColumn::Wildcard);
        self.column_decls.insert(id, kind);
        id
    }

    /// Registers a new Compute object and its ID in the AnchorContext's column_decls
    /// HashMap.
    pub fn register_compute(&mut self, compute: Compute) {
        let id = compute.id;
        let decl = ColumnDecl::Compute(Box::new(compute));
        self.column_decls.insert(id, decl);
    }

    /// Creates a new table instance and registers it in the AnchorContext's
    /// table_instances HashMap. Also generates new IDs and names for columns
    /// as needed.
    pub fn create_table_instance(&mut self, mut table_ref: TableRef) -> TableRef {
        let tiid = self.tiid.gen();

        for (col, cid) in &table_ref.columns {
            let def = ColumnDecl::RelationColumn(tiid, *cid, col.clone());
            self.column_decls.insert(*cid, def);
        }

        if table_ref.name.is_none() {
            table_ref.name = Some(self.table_name.gen())
        }

        self.table_instances.insert(tiid, table_ref.clone());
        table_ref
    }

    /// Returns the name of a column if it has been given a name already, or generates
    /// a new name for it and registers it in the AnchorContext's column_names HashMap.
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

    /// Loads column names from a pipeline of [`SqlTransform`] objects into the
    /// [`AnchorContext`]'s `column_names` HashMap.
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

    /// Determines which columns are being selected in a pipeline of SqlTransform
    /// objects, and returns their IDs in a Vec.
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

    /// Collects the tables and columns used in a pipeline of [`SqlTransform`] objects.
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
    /// Loads a [`Query`] into a new [`AnchorContext`] and returns the resulting
    /// [`AnchorContext`] and a `Relation` object representing the query.
    fn load(context: AnchorContext, query: Query) -> (AnchorContext, Relation) {
        let mut loader = QueryLoader { context };

        for t in query.tables {
            loader.load_table(t).unwrap();
        }
        let relation = loader.fold_relation(query.relation).unwrap();
        (loader.context, relation)
    }

    /// Loads a [`TableDecl`] into the [`AnchorContext`]'s `table_decls` HashMap.
    fn load_table(&mut self, table: TableDecl) -> Result<()> {
        let mut decl = fold_table(self, table)?;

        // assume name of the LocalTable that the relation is referencing
        if let RelationKind::ExternRef(table) = &decl.relation.kind {
            decl.name = Some(table.clone());
        }

        // generate name (if not present)
        if decl.name.is_none() && decl.relation.kind.as_extern_ref().is_none() {
            decl.name = Some(self.context.table_name.gen());
        }

        let sql_decl = SqlTableDecl {
            id: decl.id,
            name: decl.name,
            relation: if matches!(decl.relation.kind, RelationKind::ExternRef(_)) {
                None
            } else {
                Some(decl.relation.into())
            },
        };

        self.context.table_decls.insert(decl.id, sql_decl);
        Ok(())
    }
}

impl RqFold for QueryLoader {
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
