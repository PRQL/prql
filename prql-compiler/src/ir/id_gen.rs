use std::marker::PhantomData;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::ir_fold::fold_table;
use super::{CId, ColumnDef, IrFold, Query, TId, TableDef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdGenerator<T: From<usize>> {
    next_id: usize,
    phantom: PhantomData<T>,
}

impl<T: From<usize>> IdGenerator<T> {
    pub fn new() -> Self {
        Self::default()
    }

    fn skip(&mut self, id: usize) {
        self.next_id = self.next_id.max(id + 1);
    }

    pub fn gen(&mut self) -> T {
        let id = self.next_id;
        self.next_id += 1;
        T::from(id)
    }
}

impl<T: From<usize>> Default for IdGenerator<T> {
    fn default() -> IdGenerator<T> {
        IdGenerator {
            next_id: 0,
            phantom: PhantomData::default(),
        }
    }
}

impl IdGenerator<usize> {
    /// Returns a new id generators capable of generating new ids for given query.
    pub fn load(query: Query) -> (IdGenerator<CId>, IdGenerator<TId>, Query) {
        let mut loader = IdLoader {
            cid: IdGenerator::<CId>::default(),
            tid: IdGenerator::<TId>::default(),
        };
        let query = loader.fold_query(query).unwrap();
        (loader.cid, loader.tid, query)
    }
}
struct IdLoader {
    cid: IdGenerator<CId>,
    tid: IdGenerator<TId>,
}

impl IrFold for IdLoader {
    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        self.cid.skip(cd.id.0);

        Ok(cd)
    }

    fn fold_table(&mut self, table: TableDef) -> Result<TableDef> {
        self.tid.skip(table.id.0);

        fold_table(self, table)
    }
}
