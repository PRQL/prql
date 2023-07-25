use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};

use enum_as_inner::EnumAsInner;
use itertools::{Itertools, Position};
use serde::{Deserialize, Serialize};

use super::Ident;

/// Represents the object that is manipulated by the pipeline transforms.
/// Similar to a view in a database or a data frame.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineage {
    pub columns: Vec<LineageColumn>,

    pub inputs: Vec<LineageInput>,

    // A hack that allows name retention when applying `ExprKind::All { except }`
    #[serde(skip)]
    pub prev_columns: Vec<LineageColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageInput {
    /// Id of the node in AST that declares this input.
    pub id: usize,

    /// Local name of this input within a query.
    pub name: String,

    /// Fully qualified name of the table that provides the data for this input.
    pub table: Ident,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, EnumAsInner)]
pub enum LineageColumn {
    Single {
        name: Option<Ident>,

        // id of the defining expr (which can be actual expr or lineage input expr)
        target_id: usize,

        // if target is a relation, this is the name within the relation
        target_name: Option<String>,
    },

    /// All columns (including unknown ones) from an input (i.e. `foo_table.*`)
    All {
        input_name: String,
        except: HashSet<String>,
    },
}

impl Display for Lineage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        display_lineage(self, f, false)
    }
}

fn display_lineage(lineage: &Lineage, f: &mut Formatter, display_ids: bool) -> std::fmt::Result {
    write!(f, "[")?;
    for (pos, col) in lineage.columns.iter().with_position() {
        let is_last = matches!(pos, Position::Last | Position::Only);
        display_lineage_column(col, f, display_ids)?;
        if !is_last {
            write!(f, ", ")?;
        }
    }
    write!(f, "]")
}

fn display_lineage_column(
    col: &LineageColumn,
    f: &mut Formatter,
    display_ids: bool,
) -> std::fmt::Result {
    match col {
        LineageColumn::All { input_name, .. } => {
            write!(f, "{input_name}.*")?;
        }
        LineageColumn::Single {
            name, target_id, ..
        } => {
            if let Some(name) = name {
                write!(f, "{name}")?
            } else {
                write!(f, "?")?
            }
            if display_ids {
                write!(f, ":{target_id}")?
            }
        }
    }
    Ok(())
}
