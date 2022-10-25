//! Transform the parsed AST into a "materialized" AST, by executing functions and
//! replacing variables. The materialized AST is "flat", in the sense that it
//! contains no query-specific logic.
use std::collections::HashMap;

use anyhow::Result;

use crate::ast::TableRef;
use crate::ir::{
    fold_table, CId, ColumnDef, Expr, ExprKind, IdGenerator, IrFold, Query, TId, Table, TableExpr,
    Transform,
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
        def.name.clone()
    }

    pub fn gen_table_name(&mut self) -> String {
        let id = self.next_table_name_id;
        self.next_table_name_id += 1;

        format!("table_{id}")
    }

    fn ensure_column_name(&mut self, cid: &CId) -> String {
        let def = self.columns_defs.get_mut(cid).unwrap();

        if def.name.is_none() {
            let id = self.next_col_name_id;
            self.next_col_name_id += 1;

            def.name = Some(format!("_expr_{id}"));
        }

        def.name.clone().unwrap()
    }

    pub fn materialize_expr(&self, cid: &CId) -> Expr {
        let def = self
            .columns_defs
            .get(cid)
            .unwrap_or_else(|| panic!("missing column id {cid:?}"));
        def.expr.clone()
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

    pub fn split_pipeline(
        &mut self,
        pipeline: Vec<Transform>,
        at_position: usize,
        new_table_name: &str,
    ) -> (Vec<Transform>, Vec<Transform>) {
        let new_tid = self.ids.gen_tid();

        // define columns of the new CTE
        let mut columns_redirect = HashMap::<CId, CId>::new();
        let old_columns = self.determine_select_columns(&pipeline[0..at_position]);
        let mut new_columns = Vec::new();
        for old_cid in old_columns {
            let new_cid = self.ids.gen_cid();
            columns_redirect.insert(old_cid, new_cid);

            let old_def = self.columns_defs.get(&old_cid).unwrap();

            let new_def = ColumnDef {
                id: new_cid,
                name: old_def.name.clone(),
                expr: Expr {
                    kind: ExprKind::ExternRef {
                        variable: self.ensure_column_name(&old_cid),
                        table: Some(new_tid),
                    },
                    span: None,
                },
            };
            self.columns_defs.insert(new_cid, new_def.clone());
            self.columns_loc.insert(new_cid, new_tid);
            new_columns.push(new_def);
        }

        // define a new local table
        self.table_defs.insert(
            new_tid,
            TableDef {
                name: new_table_name.to_string(),
                expr: TableExpr::Ref(
                    TableRef::LocalTable(new_table_name.to_string()),
                    new_columns.clone(),
                ),
                columns: new_columns,
            },
        );

        // split the pipeline
        let mut first = pipeline;
        let mut second = first.split_off(at_position);

        // adjust second part: prepend from and rewrite expressions to use new columns
        second.insert(0, Transform::From(new_tid));

        let mut redirector = CidRedirector {
            redirects: columns_redirect,
        };
        let second = redirector.fold_transforms(second).unwrap();

        (first, second)
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
                        expr: Expr {
                            kind: ExprKind::ExternRef {
                                variable: "*".to_string(),
                                table: Some(table.id),
                            },
                            span: None,
                        },
                        name: None,
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

struct CidRedirector {
    redirects: HashMap<CId, CId>,
}

impl IrFold for CidRedirector {
    fn fold_cid(&mut self, cid: CId) -> Result<CId> {
        Ok(self.redirects.get(&cid).cloned().unwrap_or(cid))
    }
}
