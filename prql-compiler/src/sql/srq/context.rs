//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::{HashMap, HashSet};
use std::iter::zip;

use anyhow::Result;
use enum_as_inner::EnumAsInner;
use itertools::Itertools;
use serde::Serialize;

use crate::ast::pl::Ident;
use crate::ast::rq::{
    fold_table, CId, Compute, Query, Relation, RelationColumn, RelationKind, RqFold, TId,
    TableDecl, TableRef, Transform,
};

use crate::utils::{IdGenerator, NameGenerator};

use super::ast::{SqlRelation, SqlTransform};

/// The AnchorContext struct stores information about tables and columns, and
/// is used to generate new IDs and names.
#[derive(Default, Debug)]
pub struct AnchorContext {
    pub column_decls: HashMap<CId, ColumnDecl>,
    pub column_names: HashMap<CId, String>,

    pub table_decls: HashMap<TId, SqlTableDecl>,

    pub relation_instances: HashMap<RIId, RelationInstance>,

    pub col_name: NameGenerator,
    pub table_name: NameGenerator,

    pub cid: IdGenerator<CId>,
    pub tid: IdGenerator<TId>,
    pub riid: IdGenerator<RIId>,
}

#[derive(Debug, Clone)]
pub struct SqlTableDecl {
    #[allow(dead_code)]
    pub id: TId,

    /// Name of the table. Sometimes pull-in from RQ name hints (or database table names).
    /// Generated in postprocessing.
    pub name: Option<Ident>,

    /// Relation that still needs to be defined (usually as CTE) so it can be referenced by name.
    /// None means that it has already been defined, or was not needed to be defined in the
    /// first place.
    pub relation: RelationStatus,
}

#[derive(Debug, Clone)]
pub enum RelationStatus {
    /// Table or a common table expression. It can be referenced by name.
    Defined,

    /// Relation expression which is yet to be defined.
    NotYetDefined(RelationAdapter),
}

#[derive(Debug)]
pub struct RelationInstance {
    pub riid: RIId,

    pub table_ref: TableRef,

    /// When a pipeline is split, [CId]s from first pipeline are assigned a new
    /// [CId] in the second pipeline.
    pub cid_redirects: HashMap<CId, CId>,
}

impl RelationStatus {
    /// Analogous to [Option::take]
    pub fn take_to_define(&mut self) -> RelationStatus {
        std::mem::replace(self, RelationStatus::Defined)
    }
}

/// A relation which may have already been preprocessed.
#[derive(Debug, Clone)]
pub enum RelationAdapter {
    Rq(Relation),
    Preprocessed(Vec<SqlTransform<TableRef>>, Vec<RelationColumn>),
    Srq(SqlRelation),
}

impl From<SqlRelation> for RelationAdapter {
    fn from(rel: SqlRelation) -> Self {
        RelationAdapter::Srq(rel)
    }
}

impl From<Relation> for RelationAdapter {
    fn from(rel: Relation) -> Self {
        RelationAdapter::Rq(rel)
    }
}

/// Table instance id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct RIId(usize);

impl From<usize> for RIId {
    fn from(id: usize) -> Self {
        RIId(id)
    }
}

/// Column declaration.
#[derive(Debug, PartialEq, Clone, strum::AsRefStr, EnumAsInner)]
pub enum ColumnDecl {
    RelationColumn(RIId, CId, RelationColumn),
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
            riid: IdGenerator::new(),
            col_name: NameGenerator::new("_expr_"),
            table_name: NameGenerator::new("table_"),
            ..Default::default()
        };
        QueryLoader::load(context, query)
    }

    /// Generates a new ID and name for a wildcard column and registers it in the
    /// AnchorContext's column_decls HashMap.
    pub fn register_wildcard(&mut self, riid: RIId) -> CId {
        let id = self.cid.gen();
        let kind = ColumnDecl::RelationColumn(riid, id, RelationColumn::Wildcard);
        self.column_decls.insert(id, kind);
        id
    }

    pub fn register_compute(&mut self, compute: Compute) {
        let id = compute.id;
        let decl = ColumnDecl::Compute(Box::new(compute));
        self.column_decls.insert(id, decl);
    }

    /// Creates a new table instance and registers it in the AnchorContext's
    /// table_instances HashMap. Also generates new IDs and names for columns
    /// as needed.
    pub fn create_relation_instance(
        &mut self,
        table_ref: TableRef,
        cid_redirects: HashMap<CId, CId>,
    ) -> TableRef {
        let riid = self.riid.gen();

        for (col, cid) in &table_ref.columns {
            let def = ColumnDecl::RelationColumn(riid, *cid, col.clone());
            self.column_decls.insert(*cid, def);
        }

        let relation_instance = RelationInstance {
            riid,
            table_ref: table_ref.clone(),
            cid_redirects,
        };

        self.relation_instances.insert(riid, relation_instance);
        table_ref
    }

    // TODO: this should not return an Option
    pub fn find_relation_instance<'a>(
        &'a self,
        table_ref: &TableRef,
    ) -> Option<&'a RelationInstance> {
        let (_, cid) = table_ref.columns.first()?;
        let col_decl = self.column_decls.get(cid).unwrap();
        let (riid, _, _) = col_decl.as_relation_column().unwrap();

        Some(self.relation_instances.get(riid).unwrap())
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

    pub(super) fn load_names(
        &mut self,
        pipeline: &[SqlTransform<TableRef>],
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

    pub(super) fn determine_select_columns<T>(pipeline: &[SqlTransform<T>]) -> Vec<CId> {
        use SqlTransform::Super;

        if let Some((last, remaining)) = pipeline.split_last() {
            match last {
                Super(Transform::From(table)) => {
                    table.columns.iter().map(|(_, cid)| *cid).collect()
                }
                Super(Transform::Join { with: table, .. }) => [
                    Self::determine_select_columns(remaining),
                    table.columns.iter().map(|(_, cid)| *cid).collect_vec(),
                ]
                .concat(),
                Super(Transform::Select(cols)) => cols.clone(),
                Super(Transform::Aggregate { partition, compute }) => {
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
        pipeline: &[SqlTransform<TableRef>],
    ) -> (Vec<RIId>, HashSet<CId>) {
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
    fn load(context: AnchorContext, query: Query) -> (AnchorContext, Relation) {
        let mut loader = QueryLoader { context };

        for t in query.tables {
            loader.load_table(t).unwrap();
        }
        let relation = loader.fold_relation(query.relation).unwrap();
        (loader.context, relation)
    }

    fn load_table(&mut self, table: TableDecl) -> Result<()> {
        let decl = fold_table(self, table)?;
        let mut name = decl.name.clone().map(Ident::from_name);

        // assume name of the LocalTable that the relation is referencing
        if let RelationKind::ExternRef(table) = &decl.relation.kind {
            name = Some(table.clone());
        }

        let sql_decl = SqlTableDecl {
            id: decl.id,
            name,
            relation: if matches!(decl.relation.kind, RelationKind::ExternRef(_)) {
                // this relation can be materialized by just using table name as a reference
                // ... i.e. it's already defined.
                RelationStatus::Defined
            } else {
                // this relation should be defined when needed
                RelationStatus::NotYetDefined(decl.relation.into())
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

    fn fold_table_ref(&mut self, table_ref: TableRef) -> Result<TableRef> {
        let riid = self.context.riid.gen();

        // if table_ref.name.is_none() {
        // table_ref.name = Some(self.context.table_name.gen());
        // }

        // store
        self.context.relation_instances.insert(
            riid,
            RelationInstance {
                riid,
                table_ref: table_ref.clone(),
                cid_redirects: HashMap::new(),
            },
        );

        for (col, cid) in &table_ref.columns {
            self.context
                .column_decls
                .insert(*cid, ColumnDecl::RelationColumn(riid, *cid, col.clone()));
        }

        Ok(table_ref)
    }
}
