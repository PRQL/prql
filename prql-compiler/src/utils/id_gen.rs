use std::marker::PhantomData;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::ast::rq::{fold_table, CId, IrFold, Query, TId, TableDecl};

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
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        self.cid.skip(cid.get());

        Ok(cid)
    }

    fn fold_table(&mut self, table: TableDecl) -> Result<TableDecl> {
        self.tid.skip(table.id.get());

        fold_table(self, table)
    }
}
