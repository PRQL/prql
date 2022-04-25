use std::collections::HashSet;
use std::fmt::{Debug, Formatter, Result};

use serde::{Deserialize, Serialize};

use super::Context;
use crate::ast::ColumnSort;

/// Represents the object that is manipulated by the pipeline transforms.
/// Similar to a view in a database or a data frame.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<FrameColumn>,
    pub sort: Vec<ColumnSort<usize>>,
    pub group: Vec<(String, usize)>,

    pub tables: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FrameColumn {
    All(usize),
    Unnamed(usize),
    Named(String, usize),
}

impl Frame {
    pub fn add_column(&mut self, name: Option<String>, id: usize) {
        // remove columns with the same name
        if let Some(name) = &name {
            self.columns.retain(|c| match c {
                FrameColumn::Named(n, _) => n != name,
                _ => true,
            })
        }

        let column = if let Some(name) = name {
            FrameColumn::Named(name, id)
        } else {
            FrameColumn::Unnamed(id)
        };
        self.columns.push(column);
    }

    pub fn groups_to_columns(&mut self) {
        for (name, id) in &self.group {
            self.columns.push(FrameColumn::Named(name.clone(), *id))
        }
    }

    pub fn decls_in_use(&self) -> HashSet<usize> {
        let mut r = HashSet::new();
        for col in &self.columns {
            match col {
                FrameColumn::Unnamed(id) | FrameColumn::Named(_, id) => {
                    r.insert(*id);
                }
                _ => {}
            }
        }
        for (_, col) in &self.group {
            r.insert(*col);
        }
        r
    }

    pub fn get_column_names(&self, context: &Context) -> Vec<Option<String>> {
        self.columns
            .iter()
            .map(|col| match col {
                FrameColumn::All(namespace) => {
                    let (table, _) = &context.declarations[*namespace];
                    let table = table.as_table().map(|x| x.as_str()).unwrap_or("");
                    Some(format!("{table}.*"))
                }
                FrameColumn::Unnamed(_) => None,
                FrameColumn::Named(name, _) => Some(name.clone()),
            })
            .collect()
    }
}

impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "[")?;
        for t_col in &self.columns {
            match t_col {
                FrameColumn::All(ns) => write!(f, " {ns}.* ")?,
                FrameColumn::Named(name, id) => write!(f, " {name}:{id} ")?,
                FrameColumn::Unnamed(id) => write!(f, " {id} ")?,
            }
        }
        writeln!(f, "]")
    }
}
