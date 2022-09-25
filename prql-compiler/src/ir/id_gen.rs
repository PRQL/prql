use anyhow::Result;

use super::ir_fold::fold_table;
use super::{CId, ColumnDef, IrFold, Query, TId, Table};

#[derive(Debug, Default)]
pub struct IdGenerator {
    next_cid: usize,
    next_tid: usize,
}

impl IdGenerator {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns a new id generator capable of generating new ids for given query.
    pub fn new_for(query: Query) -> (Self, Query) {
        let mut id_gen = Self::default();
        let query = id_gen.fold_query(query).unwrap();
        (id_gen, query)
    }

    pub fn gen_cid(&mut self) -> CId {
        let id = self.next_cid;
        self.next_cid += 1;
        CId::new(id)
    }

    pub fn gen_tid(&mut self) -> TId {
        let id = self.next_tid;
        self.next_tid += 1;
        TId::new(id)
    }
}

impl IrFold for IdGenerator {
    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        self.next_cid = self.next_cid.max(cd.id.0 + 1);

        Ok(cd)
    }

    fn fold_table(&mut self, table: Table) -> Result<Table> {
        self.next_tid = self.next_tid.max(table.id.0 + 1);

        fold_table(self, table)
    }
}
