use std::marker::PhantomData;

use crate::ir::rq::{fold_table, CId, RelationalQuery, RqFold, TId, TableDecl};
use crate::Result;

#[derive(Debug, Clone)]
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
            phantom: PhantomData,
        }
    }
}

impl IdGenerator<usize> {
    /// Returns a new id generators capable of generating new ids for given query.
    pub fn load(query: RelationalQuery) -> (IdGenerator<CId>, IdGenerator<TId>, RelationalQuery) {
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

impl RqFold for IdLoader {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        self.cid.skip(cid.get());

        Ok(cid)
    }

    fn fold_table(&mut self, table: TableDecl) -> Result<TableDecl> {
        self.tid.skip(table.id.get());

        fold_table(self, table)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NameGenerator {
    prefix: &'static str,
    id: IdGenerator<usize>,
}

impl NameGenerator {
    pub fn new(prefix: &'static str) -> Self {
        NameGenerator {
            prefix,
            id: IdGenerator::new(),
        }
    }

    pub fn gen(&mut self) -> String {
        format!("{}{}", self.prefix, self.id.gen())
    }
}
