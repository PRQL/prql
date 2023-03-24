use std::{
    collections::HashSet,
    fmt::{Debug, Display, Formatter},
};

use enum_as_inner::EnumAsInner;
use itertools::{Itertools, Position};
use serde::{Deserialize, Serialize};

use super::{Expr, Ident};

/// Represents the object that is manipulated by the pipeline transforms.
/// Similar to a view in a database or a data frame.
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub columns: Vec<FrameColumn>,

    pub inputs: Vec<FrameInput>,

    // A hack that allows name retention when applying `ExprKind::All { except }`
    #[serde(skip)]
    pub prev_columns: Vec<FrameColumn>,
}

#[derive(Clone, Eq, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameInput {
    /// id of the node in AST that declares this input
    pub id: usize,

    /// local name of this input within a query
    pub name: String,

    /// fully qualified name of table that provides the data for this frame
    ///
    /// `None` means this is a literal and doesn't need a table to refer to
    pub table: Option<Ident>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum FrameColumn {
    /// All columns (including unknown ones) from an input (i.e. `foo_table.*`)
    All {
        input_name: String,
        except: HashSet<String>,
    },

    Single {
        name: Option<Ident>,
        expr_id: usize,
    },
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

impl Display for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        display_frame(self, f, false)
    }
}

impl Debug for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        display_frame(self, f, true)?;
        std::fmt::Debug::fmt(&self.inputs, f)
    }
}

fn display_frame(frame: &Frame, f: &mut Formatter, display_ids: bool) -> std::fmt::Result {
    write!(f, "[")?;
    for col in frame.columns.iter().with_position() {
        let is_last = matches!(col, Position::Last(_) | Position::Only(_));
        display_frame_column(col.into_inner(), f, display_ids)?;
        if !is_last {
            write!(f, ", ")?;
        }
    }
    write!(f, "]")
}

fn display_frame_column(
    col: &FrameColumn,
    f: &mut Formatter,
    display_ids: bool,
) -> std::fmt::Result {
    match col {
        FrameColumn::All { input_name, .. } => {
            write!(f, "{input_name}.*")?;
        }
        FrameColumn::Single { name, expr_id } => {
            if let Some(name) = name {
                write!(f, "{name}")?
            } else {
                write!(f, "?")?
            }
            if display_ids {
                write!(f, ":{expr_id}")?
            }
        }
    }
    Ok(())
}
