//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::ir::{
    fold_table, fold_table_ref, CId, ColumnDef, ColumnDefKind, IdGenerator, IrFold, Query, TId,
    TableDef, TableRef, Transform,
};

#[derive(Default)]
pub struct AnchorContext {
    pub(super) columns_defs: HashMap<CId, ColumnDef>,

    pub(super) columns_loc: HashMap<CId, TIId>,

    pub(super) table_defs: HashMap<TId, TableDef>,

    pub(super) table_instances: HashMap<TIId, TableRef>,

    next_col_name_id: u16,
    next_table_name_id: u16,

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

impl AnchorContext {
    pub fn of(query: Query) -> (Self, Query) {
        let (cid, tid, query) = IdGenerator::load(query);

        let context = AnchorContext {
            columns_defs: HashMap::new(),
            columns_loc: HashMap::new(),
            table_defs: HashMap::new(),
            table_instances: HashMap::new(),
            next_col_name_id: 0,
            next_table_name_id: 0,
            cid,
            tid,
            tiid: IdGenerator::new(),
        };
        QueryLoader::load(context, query)
    }

    pub fn get_column_name(&self, cid: &CId) -> Option<String> {
        let def = self.columns_defs.get(cid).unwrap();
        def.get_name().cloned()
    }

    pub fn gen_table_name(&mut self) -> String {
        let id = self.next_table_name_id;
        self.next_table_name_id += 1;

        format!("table_{id}")
    }

    pub fn store_new_column(&mut self, kind: ColumnDefKind, tiid: TIId) -> CId {
        let id = self.cid.gen();
        let def = ColumnDef { id, kind };
        self.columns_defs.insert(id, def);
        self.columns_loc.insert(id, tiid);
        id
    }

    pub fn ensure_column_name(&mut self, cid: &CId) -> String {
        let def = self.columns_defs.get_mut(cid).unwrap();

        match &mut def.kind {
            ColumnDefKind::Expr { name, .. } => {
                if name.is_none() {
                    let id = self.next_col_name_id;
                    self.next_col_name_id += 1;

                    *name = Some(format!("_expr_{id}"));
                }
                name.clone().unwrap()
            }
            ColumnDefKind::Wildcard => "*".to_string(),
            ColumnDefKind::ExternRef(name) => name.clone(),
        }
    }

    pub fn materialize_name(&mut self, cid: &CId) -> (Option<String>, String) {
        // TODO: figure out which columns need name and call ensure_column_name in advance
        // let col_name = self
        //     .get_column_name(cid)
        //     .expect("a column is referred by name, but it doesn't have one");
        let col_name = self.ensure_column_name(cid);

        let table_name = self.columns_loc.get(cid).map(|tiid| {
            let table = self.table_instances.get(tiid).unwrap();

            if let Some(alias) = &table.name {
                alias.clone()
            } else {
                let def = &self.table_defs[&table.source];
                def.name.clone().unwrap()
            }
        });

        (table_name, col_name)
    }

    pub fn determine_select_columns(&self, pipeline: &[Transform]) -> Vec<CId> {
        let mut columns = Vec::new();

        for transform in pipeline {
            match transform {
                Transform::From(table) => {
                    columns = table.columns.iter().map(|c| c.id).collect();
                }
                Transform::Select(cols) => columns = cols.clone(),
                Transform::Aggregate(cols) => columns = cols.clone(),
                Transform::Join { with: table, .. } => {
                    columns.extend(table.columns.iter().map(|c| c.id));
                }
                _ => {}
            }
        }

        columns
    }

    /// Returns a set of all columns of all tables in a pipeline
    pub fn collect_pipeline_inputs(&self, pipeline: &[Transform]) -> (Vec<TIId>, HashSet<CId>) {
        let mut tables = Vec::new();
        let mut columns = HashSet::new();
        for t in pipeline {
            if let Transform::From(table) | Transform::Join { with: table, .. } = t {
                // a hack to get TIId of a TableRef
                // (ideally, TIId would be saved in TableRef)
                if let Some(column) = table.columns.first() {
                    tables.push(self.columns_loc[&column.id]);
                } else {
                    panic!("table without columns?")
                }

                columns.extend(table.columns.iter().map(|c| c.id));
            }
        }
        (tables, columns)
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

impl IrFold for QueryLoader {
    fn fold_table(&mut self, table: TableDef) -> Result<TableDef> {
        let table = fold_table(self, table)?;

        self.context.table_defs.insert(table.id, table.clone());
        Ok(table)
    }

    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        self.context.columns_defs.insert(cd.id, cd.clone());
        Ok(cd)
    }

    fn fold_table_ref(&mut self, table_ref: TableRef) -> Result<TableRef> {
        let tiid = self.context.tiid.gen();

        // store
        self.context.table_instances.insert(tiid, table_ref.clone());

        // store column locations
        for col in &table_ref.columns {
            self.context.columns_loc.insert(col.id, tiid);
        }

        fold_table_ref(self, table_ref)
    }
}
