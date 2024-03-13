use crate::Result;
use itertools::Itertools;

use crate::ast::{Ident, Ty, TyTupleField};
use crate::ir::decl::{Decl, TableDecl, TableExpr};
use crate::ir::pl::{Lineage, LineageColumn, LineageInput};
use crate::semantic::{NS_DEFAULT_DB, NS_INFER};

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

        let has_wildcard = columns
            .iter()
            .any(|c| matches!(c, TyTupleField::Wildcard(_)));
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
        if let TableExpr::RelationVar(expr) = &table_decl.expr {
            if let Some(frame) = &expr.lineage {
                let wildcard_inputs = (frame.columns.iter())
                    .filter_map(|c| c.as_all())
                    .collect_vec();

                match wildcard_inputs.len() {
                    0 => return Err(format!("Cannot infer where {table_ident}.{col_name} is from")),
                    1 => {
                        let (input_id, _) = wildcard_inputs.into_iter().next().unwrap();

                        let input = frame.find_input(*input_id).unwrap();
                        let table_ident = input.table.clone();
                        self.infer_table_column(&table_ident, col_name)?;
                    }
                    _ => {
                        return Err(format!("Cannot infer where {table_ident}.{col_name} is from. It could be any of {wildcard_inputs:?}"))
                    }
                }
            }
        }

        Ok(())
    }

    /// Converts a identifier that points to a table declaration to lineage of that table.
    pub fn lineage_of_table_decl(
        &mut self,
        table_fq: &Ident,
        input_name: String,
        input_id: usize,
    ) -> Lineage {
        let table_decl = self.root_mod.module.get(table_fq).unwrap();
        let TableDecl { ty, .. } = table_decl.kind.as_table_decl().unwrap();

        // TODO: can this panic?
        let columns = ty.as_ref().unwrap().as_relation().unwrap();

        let mut instance_frame = Lineage {
            inputs: vec![LineageInput {
                id: input_id,
                name: input_name.clone(),
                table: table_fq.clone(),
            }],
            columns: Vec::new(),
            ..Default::default()
        };

        for col in columns {
            let col = match col {
                TyTupleField::Wildcard(_) => LineageColumn::All {
                    input_id,
                    except: columns
                        .iter()
                        .flat_map(|c| c.as_single().map(|x| x.0).cloned().flatten())
                        .collect(),
                },
                TyTupleField::Single(col_name, _) => LineageColumn::Single {
                    name: col_name
                        .clone()
                        .map(|col_name| Ident::from_path(vec![input_name.clone(), col_name])),
                    target_id: input_id,
                    target_name: col_name.clone(),
                },
            };
            instance_frame.columns.push(col);
        }

        log::debug!("instanced table {table_fq} as {instance_frame:?}");
        instance_frame
    }

    /// Declares a new table for a relation literal.
    /// This is needed for column inference to work properly.
    pub(super) fn declare_table_for_literal(
        &mut self,
        input_id: usize,
        columns: Option<Vec<TyTupleField>>,
        name_hint: Option<String>,
    ) -> Lineage {
        let id = input_id;
        let global_name = format!("_literal_{}", id);

        // declare a new table in the `default_db` module
        let default_db_ident = Ident::from_name(NS_DEFAULT_DB);
        let default_db = self.root_mod.module.get_mut(&default_db_ident).unwrap();
        let default_db = default_db.kind.as_module_mut().unwrap();

        let infer_default = default_db.get(&Ident::from_name(NS_INFER)).unwrap().clone();
        let mut infer_default = *infer_default.kind.into_infer().unwrap();

        let table_decl = infer_default.as_table_decl_mut().unwrap();
        table_decl.expr = TableExpr::None;

        if let Some(columns) = columns {
            table_decl.ty = Some(Ty::relation(columns));
        }

        default_db
            .names
            .insert(global_name.clone(), Decl::from(infer_default));

        // produce a frame of that table
        let input_name = name_hint.unwrap_or_else(|| global_name.clone());
        let table_fq = default_db_ident + Ident::from_name(global_name);
        self.lineage_of_table_decl(&table_fq, input_name, id)
    }
}
