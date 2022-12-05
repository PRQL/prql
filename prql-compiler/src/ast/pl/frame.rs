use std::fmt::{Debug, Display, Formatter};

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
    /// Used for `foo_table.*`
    Wildcard {
        input_name: String,
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

impl Frame {
    pub fn push_column(&mut self, col_name: Option<String>, id: usize) {
        // remove names from columns with the same name
        if col_name.is_some() {
            for c in &mut self.columns {
                if let FrameColumn::Single { name, .. } = c {
                    if name.as_ref().map(|i| &i.name) == col_name.as_ref() {
                        *name = None;
                    }
                }
            }
        }

        self.columns.push(FrameColumn::Single {
            name: col_name.map(Ident::from_name),
            expr_id: id,
        });
    }

    pub fn apply_assigns(&mut self, assigns: &[Expr]) {
        for expr in assigns {
            let id = expr.id.unwrap();

            let name = expr
                .alias
                .clone()
                .or_else(|| expr.kind.as_ident().cloned().map(|x| x.name));

            self.push_column(name, id);
        }
    }

    pub fn find_input(&self, input_name: &str) -> Option<&FrameInput> {
        self.inputs.iter().find(|i| i.name == input_name)
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
        FrameColumn::Wildcard { input_name } => {
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

impl std::ops::Add<Frame> for Frame {
    type Output = Frame;

    fn add(mut self, rhs: Frame) -> Frame {
        self.columns.extend(rhs.columns);
        self.inputs.extend(rhs.inputs);
        self
    }
}
