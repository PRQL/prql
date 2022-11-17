use std::fmt::{Debug, Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::ast::Expr;

/// Represents the object that is manipulated by the pipeline transforms.
/// Similar to a view in a database or a data frame.
#[derive(Clone, Default, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<FrameColumn>,
    pub sort: Vec<ColumnSort<usize>>,
    pub tables: Vec<usize>,
}

/// Columns we know about in a Frame. The usize value represents the table id.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum FrameColumn {
    /// Used for `foo_table.*`
    All(usize),
    /// Used for `derive a + b` (new column has no name)
    Unnamed(usize),
    Named(String, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnSort<T = Expr> {
    pub direction: SortDirection,
    pub column: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        SortDirection::Asc
    }
}

impl Frame {
    pub fn unknown(table_id: usize) -> Self {
        Frame {
            columns: vec![FrameColumn::All(table_id)],
            sort: Vec::new(),
            tables: vec![],
        }
    }

    pub fn push_column(&mut self, name: Option<String>, id: usize) {
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

    pub fn apply_assigns(&mut self, assigns: &[Expr]) {
        for expr in assigns {
            let id = expr.declared_at.unwrap();
            let name = expr
                .alias
                .clone()
                .or_else(|| expr.kind.as_ident().cloned().map(|x| x.name));

            self.push_column(name, id);
        }
    }
}

impl Display for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        for t_col in &self.columns {
            match t_col {
                FrameColumn::All(ns) => write!(f, " {ns}.* ")?,
                FrameColumn::Named(name, id) => write!(f, " {name}:{id} ")?,
                FrameColumn::Unnamed(id) => write!(f, " {id} ")?,
            }
        }
        write!(f, "]")
    }
}

impl PartialEq<usize> for FrameColumn {
    fn eq(&self, other: &usize) -> bool {
        match self {
            FrameColumn::All(_) => false,
            FrameColumn::Unnamed(id) | FrameColumn::Named(_, id) => id == other,
        }
    }
}
