use std::marker::PhantomData;

use anyhow::Result;

use crate::ast::rq::{fold_table, CId, IrFold, Query, TId, TableDecl};

#[derive(Debug, Clone)]
pub struct IdGenerator<T: From<usize>> {
    next_id: usize,
    phantom: PhantomData<T>,
}

impl<T: From<usize>> IdGenerator<T> {
    pub fn new() -> Self {
        Self::default()
    }

    // We could implement this with `skip_while`, but this is just as concise.
    fn skip_to(&mut self, id: usize) {
        self.next_id = self.next_id.max(id + 1);
    }
}

impl<T: From<usize>> Iterator for IdGenerator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.next_id;
        self.next_id += 1;
        Some(T::from(id))
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
        self.cid.skip_to(cid.get());

        Ok(cid)
    }

    fn fold_table(&mut self, table: TableDecl) -> Result<TableDecl> {
        self.tid.skip_to(table.id.get());

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
        format!("{}{}", self.prefix, self.id.next().unwrap())
    }
}
