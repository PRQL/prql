//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::ir::{
    fold_table, CId, ColumnDef, ColumnDefKind, Expr, ExprKind, IdGenerator, IrFold, Query, TId,
    Table, TableExpr, Transform,
};

#[derive(Default)]
pub struct AnchorContext {
    pub(super) columns_defs: HashMap<CId, ColumnDef>,

    pub(super) columns_loc: HashMap<CId, TId>,

    pub(super) table_defs: HashMap<TId, TableDef>,

    next_col_name_id: u16,
    next_table_name_id: u16,

    pub(super) ids: IdGenerator,
}

pub struct TableDef {
    /// How to reference this table
    pub name: String,

    pub columns: Vec<ColumnDef>,

    /// How to materialize in FROM/WITH clauses
    pub expr: TableExpr,
}

impl AnchorContext {
    pub fn of(query: Query) -> (Self, Query) {
        let (ids, query) = IdGenerator::new_for(query);

        let context = AnchorContext {
            columns_defs: HashMap::new(),
            columns_loc: HashMap::new(),
            table_defs: HashMap::new(),
            next_col_name_id: 0,
            next_table_name_id: 0,
            ids,
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
            ColumnDefKind::Wildcard(_) => "*".to_string(),
            ColumnDefKind::ExternRef(name) => name.clone(),
        }
    }

    pub fn materialize_expr(&self, cid: &CId) -> Expr {
        let def = self
            .columns_defs
            .get(cid)
            .unwrap_or_else(|| panic!("missing column id {cid:?}"));

        match &def.kind {
            ColumnDefKind::Expr { expr, .. } => expr.clone(),
            _ => Expr {
                kind: ExprKind::ColumnRef(*cid),
                span: None,
            },
        }
    }

    #[allow(dead_code)]
    pub fn materialize_exprs(&self, cids: &[CId]) -> Vec<Expr> {
        cids.iter().map(|cid| self.materialize_expr(cid)).collect()
    }

    pub fn materialize_name(&mut self, cid: &CId) -> (Option<String>, String) {
        // TODO: figure out which columns need name and call ensure_column_name in advance
        // let col_name = self
        //     .get_column_name(cid)
        //     .expect("a column is referred by name, but it doesn't have one");
        let col_name = self.ensure_column_name(cid);

        let table = self.columns_loc.get(cid).map(|tid| {
            let table = self.table_defs.get(tid).unwrap();

            table.name.clone()
        });
        (table, col_name)
    }

    pub fn determine_select_columns(&self, pipeline: &[Transform]) -> Vec<CId> {
        let mut columns = Vec::new();

        for transform in pipeline {
            match transform {
                Transform::From(tid) => {
                    let table_def = &self.table_defs.get(tid).unwrap();
                    columns = table_def.columns.iter().map(|c| c.id).collect();
                }
                Transform::Select(cols) => columns = cols.clone(),
                Transform::Aggregate(cols) => columns = cols.clone(),
                Transform::Join { with, .. } => {
                    let table_def = &self.table_defs.get(with).unwrap();
                    columns.extend(table_def.columns.iter().map(|c| c.id));
                }
                _ => {}
            }
        }

        columns
    }

    /// Returns a set of all columns of all tables in a pipeline
    pub fn collect_pipeline_inputs(&self, pipeline: &[Transform]) -> (Vec<TId>, HashSet<CId>) {
        let mut tables = Vec::new();
        let mut columns = HashSet::new();
        for t in pipeline {
            if let Transform::From(tid) | Transform::Join { with: tid, .. } = t {
                tables.push(*tid);
                columns.extend(self.table_defs[tid].columns.iter().map(|c| c.id));
            }
        }
        (tables, columns)
    }
}

/// Loads info about [Query] into [AnchorContext]
struct QueryLoader {
    context: AnchorContext,

    current_table: Option<TId>,
}

impl QueryLoader {
    fn load(context: AnchorContext, query: Query) -> (AnchorContext, Query) {
        let mut loader = QueryLoader {
            context,
            current_table: None,
        };
        // fold query
        let query = loader.fold_query(query).unwrap();
        let mut context = loader.context;

        // move tables into Context
        for table in query.tables.clone() {
            let name = table.name.as_ref().unwrap().clone();

            let columns = match &table.expr {
                TableExpr::Ref(_, cols) => cols.clone(),
                TableExpr::Pipeline(_) => {
                    let star_col = ColumnDef {
                        id: context.ids.gen_cid(),
                        kind: ColumnDefKind::Wildcard(table.id),
                    };
                    context.columns_loc.insert(star_col.id, table.id);
                    context.columns_defs.insert(star_col.id, star_col.clone());

                    vec![star_col]
                }
            };

            let table_def = TableDef {
                name,
                columns,
                expr: table.expr,
            };
            context.table_defs.insert(table.id, table_def);
        }

        (context, query)
    }
}

impl IrFold for QueryLoader {
    fn fold_table(&mut self, table: Table) -> Result<Table> {
        self.current_table = Some(table.id);

        fold_table(self, table)
    }

    fn fold_column_def(&mut self, cd: ColumnDef) -> Result<ColumnDef> {
        self.context.columns_defs.insert(cd.id, cd.clone());

        if let Some(current_table) = self.current_table {
            self.context.columns_loc.insert(cd.id, current_table);
        }

        Ok(cd)
    }
}
