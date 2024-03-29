use crate::Result;

use crate::ast::{Ident, Ty, TyTupleField};
use crate::ir::decl::{TableDecl, TableExpr};

use super::Resolver;

impl Resolver<'_> {
    pub fn infer_table_column(
        &mut self,
        table_ident: &Ident,
        col_name: &str,
    ) -> Result<(), String> {
        let table = self.root_mod.module.get_mut(table_ident).unwrap();
        let table_decl = table.kind.as_table_decl_mut().unwrap();

        let Some(columns) = table_decl.ty.as_mut().and_then(|t| t.as_relation_mut()) else {
            return Err(format!("Variable {table_ident:?} is not a relation."));
        };

        let has_wildcard = columns.iter().any(|c| matches!(c, TyTupleField::Unpack(_)));
        if !has_wildcard {
            return Err(format!("Table {table_ident:?} does not have wildcard."));
        }

        let exists = columns.iter().any(|c| match c {
            TyTupleField::Single(Some(n), _) => n == col_name,
            _ => false,
        });
        if exists {
            return Ok(());
        }

        columns.push(TyTupleField::Single(Some(col_name.to_string()), None));

        // also add into input tables of this table expression
        if let TableExpr::RelationVar(_) = &table_decl.expr {
            // TODO
            // if let Some(frame) = &expr.lineage {
            //     let wildcard_inputs = (frame.columns.iter())
            //         .filter_map(|c| c.as_all())
            //         .collect_vec();

            //     match wildcard_inputs.len() {
            //         0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
            //         1 => {
            //             let (input_id, _) = wildcard_inputs.into_iter().next().unwrap();

            //             let input = frame.find_input(*input_id).unwrap();
            //             let table_ident = input.table.clone();
            //             self.infer_table_column(&table_ident, col_name)?;
            //         }
            //         _ => {
            //             return Err(format!("Cannot infer where {table_ident}.{col_name} is from. It could be any of {wildcard_inputs:?}"))
            //         }
            //     }
            // }
        }

        Ok(())
    }

    /// Converts a identifier that points to a table declaration to lineage of that table.
    pub fn ty_of_table_decl(&mut self, table_fq: &Ident) -> Ty {
        let table_decl = self.root_mod.module.get(table_fq).unwrap();
        let TableDecl { ty, .. } = table_decl.kind.as_table_decl().unwrap();

        ty.clone()
            .expect("a referenced relation to have its type resolved")
    }
}
